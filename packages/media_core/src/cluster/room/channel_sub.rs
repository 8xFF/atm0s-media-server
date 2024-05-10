//! Channel Subscriber handle logic for viewer. This module takecare sending Sub or Unsub, and also feedback
//!

use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
    time::Instant,
};

use atm0s_sdn::{
    features::pubsub::{self, ChannelControl, ChannelId, Feedback},
    NodeId,
};
use derivative::Derivative;
use media_server_protocol::{
    endpoint::{PeerId, TrackName},
    media::MediaPacket,
};
use sans_io_runtime::{return_if_none, TaskSwitcherChild};

use crate::{
    cluster::{id_generator, ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterRoomHash},
    transport::LocalTrackId,
};

const BITRATE_FEEDBACK_INTERVAL: u16 = 100; //100 ms
const BITRATE_FEEDBACK_TIMEOUT: u16 = 2000; //2 seconds

const KEYFRAME_FEEDBACK_INTERVAL: u16 = 1000; //100 ms
const KEYFRAME_FEEDBACK_TIMEOUT: u16 = 2000; //2 seconds

const BITRATE_FEEDBACK_KIND: u8 = 0;
const KEYFRAME_FEEDBACK_KIND: u8 = 1;

#[derive(Debug, PartialEq, Eq)]
pub enum Output<Owner> {
    Endpoint(Vec<Owner>, ClusterEndpointEvent),
    Pubsub(pubsub::Control),
}

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
struct ChannelContainer<Owner> {
    owners: Vec<(Owner, LocalTrackId)>,
    bitrate_fbs: HashMap<Owner, (Instant, Feedback)>,
}

pub struct RoomChannelSubscribe<Owner> {
    room: ClusterRoomHash,
    channels: HashMap<ChannelId, ChannelContainer<Owner>>,
    subscribers: HashMap<(Owner, LocalTrackId), (ChannelId, PeerId, TrackName)>,
    queue: VecDeque<Output<Owner>>,
}

impl<Owner: Hash + Eq + Copy + Debug> RoomChannelSubscribe<Owner> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            channels: HashMap::new(),
            subscribers: HashMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn on_channel_relay_changed(&mut self, channel: ChannelId, _relay: NodeId) {
        let channel_container = return_if_none!(self.channels.get(&channel));
        log::info!(
            "[ClusterRoom {}/Subscribers] cluster: channel {channel} source changed => fire event to {:?}",
            self.room,
            channel_container.owners
        );
        for (owner, track) in &channel_container.owners {
            self.queue
                .push_back(Output::Endpoint(vec![*owner], ClusterEndpointEvent::LocalTrack(*track, ClusterLocalTrackEvent::SourceChanged)))
        }
    }

    pub fn on_channel_data(&mut self, channel: ChannelId, data: Vec<u8>) {
        let pkt = return_if_none!(MediaPacket::deserialize(&data));
        let channel_container = return_if_none!(self.channels.get(&channel));
        log::trace!(
            "[ClusterRoom {}/Subscribers] on channel media meta {:?} seq {} to {} subscribers",
            self.room,
            pkt.meta,
            pkt.seq,
            channel_container.owners.len()
        );
        for (owner, track) in &channel_container.owners {
            self.queue.push_back(Output::Endpoint(
                vec![*owner],
                ClusterEndpointEvent::LocalTrack(*track, ClusterLocalTrackEvent::Media(*channel, pkt.clone())),
            ))
        }
    }

    pub fn on_track_subscribe(&mut self, owner: Owner, track: LocalTrackId, target_peer: PeerId, target_track: TrackName) {
        let channel_id: ChannelId = id_generator::gen_channel_id(self.room, &target_peer, &target_track);
        log::info!(
            "[ClusterRoom {}/Subscribers] owner {:?} track {track} subscribe peer {target_peer} track {target_track}), channel: {channel_id}",
            self.room,
            owner
        );
        self.subscribers.insert((owner, track), (channel_id, target_peer, target_track));
        let channel_container = self.channels.entry(channel_id).or_insert(Default::default());
        channel_container.owners.push((owner, track));
        if channel_container.owners.len() == 1 {
            log::info!("[ClusterRoom {}/Subscribers] first subscriber => Sub channel {channel_id}", self.room);
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto)));
        }
    }

    pub fn on_track_request_key(&mut self, owner: Owner, track: LocalTrackId) {
        let (channel_id, peer, track) = return_if_none!(self.subscribers.get(&(owner, track)));
        log::info!("[ClusterRoom {}/Subscribers] request key-frame {channel_id} {peer} {track}", self.room);
        self.queue.push_back(Output::Pubsub(pubsub::Control(
            *channel_id,
            ChannelControl::FeedbackAuto(Feedback::simple(KEYFRAME_FEEDBACK_KIND, 1, KEYFRAME_FEEDBACK_INTERVAL, KEYFRAME_FEEDBACK_TIMEOUT)),
        )));
    }

    pub fn on_track_desired_bitrate(&mut self, now: Instant, owner: Owner, track: LocalTrackId, bitrate: u64) {
        let (channel_id, _peer, _track) = return_if_none!(self.subscribers.get(&(owner, track)));
        let channel_container = return_if_none!(self.channels.get_mut(channel_id));
        let fb = Feedback::simple(BITRATE_FEEDBACK_KIND, bitrate, BITRATE_FEEDBACK_INTERVAL, BITRATE_FEEDBACK_TIMEOUT);
        channel_container.bitrate_fbs.insert(owner, (now, fb));

        //clean if if timeout
        channel_container
            .bitrate_fbs
            .retain(|_, (ts, _)| now.duration_since(*ts).as_millis() < BITRATE_FEEDBACK_TIMEOUT as u128);

        //sum all fbs
        let mut sum_fb = None;
        for (_, fb) in channel_container.bitrate_fbs.values() {
            if let Some(sum_fb) = &mut sum_fb {
                *sum_fb = *sum_fb + *fb;
            } else {
                sum_fb = Some(fb.clone());
            }
        }
        log::debug!("[ClusterRoom {}/Subscribers] channel {channel_id} setting desired bitrate {:?}", self.room, sum_fb);
        self.queue
            .push_back(Output::Pubsub(pubsub::Control(*channel_id, ChannelControl::FeedbackAuto(return_if_none!(sum_fb)))));
    }

    pub fn on_track_unsubscribe(&mut self, owner: Owner, track: LocalTrackId) {
        let (channel_id, target_peer, target_track) = return_if_none!(self.subscribers.remove(&(owner, track)));
        log::info!(
            "[ClusterRoom {}/Subscribers] owner {:?} track {track} unsubscribe from source {target_peer} {target_track}, channel {channel_id}",
            self.room,
            owner
        );
        let channel_container = return_if_none!(self.channels.get_mut(&channel_id));
        let (index, _) = return_if_none!(channel_container.owners.iter().enumerate().find(|e| e.1.eq(&(owner, track))));
        channel_container.owners.swap_remove(index);

        if channel_container.owners.is_empty() {
            self.channels.remove(&channel_id);
            log::info!("[ClusterRoom {}/Subscribers] last unsubscriber => Unsub channel {channel_id}", self.room);
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto)));
        }
    }
}

impl<Owner: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Owner>> for RoomChannelSubscribe<Owner> {
    type Time = Instant;
    fn pop_output(&mut self, now: Instant) -> Option<Output<Owner>> {
        self.queue.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use atm0s_sdn::features::pubsub::{ChannelControl, Control, Feedback};
    use media_server_protocol::{
        endpoint::{PeerId, TrackName},
        media::{MediaMeta, MediaPacket},
    };
    use sans_io_runtime::TaskSwitcherChild;

    use crate::{
        cluster::{
            room::channel_sub::{BITRATE_FEEDBACK_INTERVAL, BITRATE_FEEDBACK_KIND, BITRATE_FEEDBACK_TIMEOUT, KEYFRAME_FEEDBACK_INTERVAL, KEYFRAME_FEEDBACK_KIND, KEYFRAME_FEEDBACK_TIMEOUT},
            ClusterEndpointEvent, ClusterLocalTrackEvent,
        },
        transport::LocalTrackId,
    };

    use super::id_generator::gen_channel_id;
    use super::{Output, RoomChannelSubscribe};

    pub fn fake_audio() -> MediaPacket {
        MediaPacket {
            ts: 0,
            seq: 0,
            marker: true,
            nackable: false,
            layers: None,
            meta: MediaMeta::Opus { audio_level: None },
            data: vec![1, 2, 3, 4],
        }
    }

    //TODO First Subscribe channel should sending Sub
    //TODO Last Unsubscribe channel should sending Unsub
    #[test]
    fn normal_sub_ubsub() {
        let room = 1.into();
        let mut subscriber = RoomChannelSubscribe::<u8>::new(room);

        let owner = 2;
        let track = LocalTrackId(3);
        let target_peer: PeerId = "peer2".to_string().into();
        let target_track: TrackName = "audio_main".to_string().into();
        let channel_id = gen_channel_id(room, &target_peer, &target_track);
        subscriber.on_track_subscribe(owner, track, target_peer.clone(), target_track.clone());
        assert_eq!(subscriber.pop_output(Instant::now()), Some(Output::Pubsub(Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(subscriber.pop_output(Instant::now()), None);

        let pkt = fake_audio();
        subscriber.on_channel_data(channel_id, pkt.serialize());
        assert_eq!(
            subscriber.pop_output(Instant::now()),
            Some(Output::Endpoint(vec![owner], ClusterEndpointEvent::LocalTrack(track, ClusterLocalTrackEvent::Media(*channel_id, pkt))))
        );
        assert_eq!(subscriber.pop_output(Instant::now()), None);

        subscriber.on_track_unsubscribe(owner, track);
        assert_eq!(subscriber.pop_output(Instant::now()), Some(Output::Pubsub(Control(channel_id, ChannelControl::UnsubAuto))));
        assert_eq!(subscriber.pop_output(Instant::now()), None);
    }

    //TODO Sending key-frame request
    #[test]
    fn send_key_frame() {
        let room = 1.into();
        let mut subscriber = RoomChannelSubscribe::<u8>::new(room);

        let owner = 2;
        let track = LocalTrackId(3);
        let target_peer: PeerId = "peer2".to_string().into();
        let target_track: TrackName = "audio_main".to_string().into();
        let channel_id = gen_channel_id(room, &target_peer, &target_track);
        subscriber.on_track_subscribe(owner, track, target_peer.clone(), target_track.clone());
        assert_eq!(subscriber.pop_output(Instant::now()), Some(Output::Pubsub(Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(subscriber.pop_output(Instant::now()), None);

        subscriber.on_track_request_key(owner, track);
        assert_eq!(
            subscriber.pop_output(Instant::now()),
            Some(Output::Pubsub(Control(
                channel_id,
                ChannelControl::FeedbackAuto(Feedback::simple(KEYFRAME_FEEDBACK_KIND, 1, KEYFRAME_FEEDBACK_INTERVAL, KEYFRAME_FEEDBACK_TIMEOUT))
            )))
        );
        assert_eq!(subscriber.pop_output(Instant::now()), None);
    }

    //TODO Sending bitrate request single sub
    #[test]
    fn send_bitrate_limit_speed() {
        let room = 1.into();
        let mut subscriber = RoomChannelSubscribe::<u8>::new(room);

        let owner1 = 2;
        let track1 = LocalTrackId(3);
        let target_peer: PeerId = "peer2".to_string().into();
        let target_track: TrackName = "audio_main".to_string().into();
        let channel_id = gen_channel_id(room, &target_peer, &target_track);
        subscriber.on_track_subscribe(owner1, track1, target_peer.clone(), target_track.clone());
        assert_eq!(subscriber.pop_output(Instant::now()), Some(Output::Pubsub(Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(subscriber.pop_output(Instant::now()), None);

        let mut now = Instant::now();

        subscriber.on_track_desired_bitrate(now, owner1, track1, 1000);
        assert_eq!(
            subscriber.pop_output(Instant::now()),
            Some(Output::Pubsub(Control(
                channel_id,
                ChannelControl::FeedbackAuto(Feedback::simple(BITRATE_FEEDBACK_KIND, 1000, BITRATE_FEEDBACK_INTERVAL, BITRATE_FEEDBACK_TIMEOUT))
            )))
        );
        assert_eq!(subscriber.pop_output(now), None);

        // more local track sub that channel
        let owner2 = 3;
        let track2 = LocalTrackId(4);
        subscriber.on_track_subscribe(owner2, track2, target_peer.clone(), target_track.clone());
        assert_eq!(subscriber.pop_output(Instant::now()), None);

        // more feedback from local track2
        now += Duration::from_millis(100);
        subscriber.on_track_desired_bitrate(now, owner2, track2, 2000);
        assert_eq!(
            subscriber.pop_output(Instant::now()),
            Some(Output::Pubsub(Control(
                channel_id,
                ChannelControl::FeedbackAuto(Feedback {
                    kind: BITRATE_FEEDBACK_KIND,
                    count: 2,
                    max: 2000,
                    min: 1000,
                    sum: 3000,
                    interval_ms: BITRATE_FEEDBACK_INTERVAL,
                    timeout_ms: BITRATE_FEEDBACK_TIMEOUT
                })
            )))
        );
        assert_eq!(subscriber.pop_output(now), None);

        //now last update from track2 after long time cause track1 feedback will be timeout
        now += Duration::from_millis(BITRATE_FEEDBACK_TIMEOUT as u64 - 100);
        subscriber.on_track_desired_bitrate(now, owner2, track2, 3000);
        assert_eq!(
            subscriber.pop_output(Instant::now()),
            Some(Output::Pubsub(Control(
                channel_id,
                ChannelControl::FeedbackAuto(Feedback {
                    kind: BITRATE_FEEDBACK_KIND,
                    count: 1,
                    max: 3000,
                    min: 3000,
                    sum: 3000,
                    interval_ms: BITRATE_FEEDBACK_INTERVAL,
                    timeout_ms: BITRATE_FEEDBACK_TIMEOUT
                })
            )))
        );
        assert_eq!(subscriber.pop_output(now), None);
    }
}
