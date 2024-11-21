//!
//! Channel Subscriber handle logic for viewer. This module takecare sending Sub or Unsub, and also feedback
//!

use std::{collections::VecDeque, fmt::Debug, hash::Hash, time::Instant};

use atm0s_sdn::{
    features::pubsub::{self, ChannelControl, ChannelId, Feedback},
    NodeId,
};
use derivative::Derivative;
use indexmap::IndexMap;
use media_server_protocol::{
    endpoint::{PeerId, TrackName},
    media::MediaPacket,
};
use media_server_utils::Count;
use sans_io_runtime::{return_if_none, TaskSwitcherChild};

use crate::{
    cluster::{id_generator, ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterRoomHash},
    transport::LocalTrackId,
};

use super::Output;

const BITRATE_FEEDBACK_INTERVAL: u16 = 100; //100 ms
const BITRATE_FEEDBACK_TIMEOUT: u16 = 2000; //2 seconds

const KEYFRAME_FEEDBACK_INTERVAL: u16 = 1000; //100 ms
const KEYFRAME_FEEDBACK_TIMEOUT: u16 = 2000; //2 seconds

const BITRATE_FEEDBACK_KIND: u8 = 0;
const KEYFRAME_FEEDBACK_KIND: u8 = 1;

#[derive(Derivative, Debug)]
#[derivative(Default(bound = ""))]
struct ChannelContainer<Endpoint: Debug> {
    endpoints: Vec<(Endpoint, LocalTrackId)>,
    bitrate_fbs: IndexMap<Endpoint, (Instant, Feedback)>,
}

#[derive(Debug)]
pub struct RoomChannelSubscribe<Endpoint: Debug> {
    _c: Count<Self>,
    room: ClusterRoomHash,
    channels: IndexMap<ChannelId, ChannelContainer<Endpoint>>,
    subscribers: IndexMap<(Endpoint, LocalTrackId), (ChannelId, PeerId, TrackName)>,
    queue: VecDeque<Output<Endpoint>>,
}

impl<Endpoint: Debug + Hash + Eq + Copy + Debug> RoomChannelSubscribe<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            _c: Default::default(),
            room,
            channels: IndexMap::new(),
            subscribers: IndexMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn on_track_relay_changed(&mut self, channel: ChannelId, _relay: NodeId) {
        let channel_container = return_if_none!(self.channels.get(&channel));
        log::info!(
            "[ClusterRoom {}/Subscribers] cluster: channel {channel} source changed => fire event to {:?}",
            self.room,
            channel_container.endpoints
        );
        for (endpoint, track) in &channel_container.endpoints {
            self.queue
                .push_back(Output::Endpoint(vec![*endpoint], ClusterEndpointEvent::LocalTrack(*track, ClusterLocalTrackEvent::RelayChanged)))
        }
    }

    pub fn on_track_data(&mut self, channel: ChannelId, data: Vec<u8>) {
        let pkt = return_if_none!(MediaPacket::deserialize(&data));
        let channel_container = return_if_none!(self.channels.get(&channel));
        log::trace!(
            "[ClusterRoom {}/Subscribers] on channel media meta {:?} seq {} to {} subscribers",
            self.room,
            pkt.meta,
            pkt.seq,
            channel_container.endpoints.len()
        );
        for (endpoint, track) in &channel_container.endpoints {
            self.queue.push_back(Output::Endpoint(
                vec![*endpoint],
                ClusterEndpointEvent::LocalTrack(*track, ClusterLocalTrackEvent::Media(*channel, pkt.clone())),
            ))
        }
    }

    pub fn on_track_subscribe(&mut self, endpoint: Endpoint, track: LocalTrackId, target_peer: PeerId, target_track: TrackName) {
        let channel_id: ChannelId = id_generator::gen_track_channel_id(self.room, &target_peer, &target_track);
        log::info!(
            "[ClusterRoom {}/Subscribers] endpoint {:?} track {track} subscribe peer {target_peer} track {target_track}), channel: {channel_id}",
            self.room,
            endpoint
        );
        self.subscribers.insert((endpoint, track), (channel_id, target_peer, target_track));
        let channel_container = self.channels.entry(channel_id).or_default();
        channel_container.endpoints.push((endpoint, track));
        if channel_container.endpoints.len() == 1 {
            log::info!("[ClusterRoom {}/Subscribers] first subscriber => Sub channel {channel_id}", self.room);
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto)));
        }
    }

    pub fn on_track_request_key(&mut self, endpoint: Endpoint, track: LocalTrackId) {
        let (channel_id, peer, track) = return_if_none!(self.subscribers.get(&(endpoint, track)));
        log::info!("[ClusterRoom {}/Subscribers] request key-frame {channel_id} {peer} {track}", self.room);
        self.queue.push_back(Output::Pubsub(pubsub::Control(
            *channel_id,
            ChannelControl::FeedbackAuto(Feedback::simple(KEYFRAME_FEEDBACK_KIND, 1, KEYFRAME_FEEDBACK_INTERVAL, KEYFRAME_FEEDBACK_TIMEOUT)),
        )));
    }

    pub fn on_track_desired_bitrate(&mut self, now: Instant, endpoint: Endpoint, track: LocalTrackId, bitrate: u64) {
        let (channel_id, _peer, _track) = return_if_none!(self.subscribers.get(&(endpoint, track)));
        let channel_container = return_if_none!(self.channels.get_mut(channel_id));
        let fb = Feedback::simple(BITRATE_FEEDBACK_KIND, bitrate, BITRATE_FEEDBACK_INTERVAL, BITRATE_FEEDBACK_TIMEOUT);
        channel_container.bitrate_fbs.insert(endpoint, (now, fb));

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
                sum_fb = Some(*fb);
            }
        }
        log::debug!("[ClusterRoom {}/Subscribers] channel {channel_id} setting desired bitrate {:?}", self.room, sum_fb);
        self.queue
            .push_back(Output::Pubsub(pubsub::Control(*channel_id, ChannelControl::FeedbackAuto(return_if_none!(sum_fb)))));
    }

    pub fn on_track_unsubscribe(&mut self, endpoint: Endpoint, track: LocalTrackId) {
        let (channel_id, target_peer, target_track) = return_if_none!(self.subscribers.swap_remove(&(endpoint, track)));
        log::info!(
            "[ClusterRoom {}/Subscribers] endpoint {:?} track {track} unsubscribe from source {target_peer} {target_track}, channel {channel_id}",
            self.room,
            endpoint
        );
        let channel_container = return_if_none!(self.channels.get_mut(&channel_id));
        let (index, _) = return_if_none!(channel_container.endpoints.iter().enumerate().find(|e| e.1.eq(&(endpoint, track))));
        channel_container.endpoints.swap_remove(index);

        if channel_container.endpoints.is_empty() {
            self.channels.swap_remove(&channel_id);
            log::info!("[ClusterRoom {}/Subscribers] last unsubscriber => Unsub channel {channel_id}", self.room);
            self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::UnsubAuto)));
        }
    }
}

impl<Endpoint: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Endpoint>> for RoomChannelSubscribe<Endpoint> {
    type Time = ();

    fn is_empty(&self) -> bool {
        self.subscribers.is_empty() && self.channels.is_empty() && self.queue.is_empty()
    }

    fn empty_event(&self) -> Output<Endpoint> {
        Output::OnResourceEmpty
    }

    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}

impl<Endpoint: Debug> Drop for RoomChannelSubscribe<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterRoom {}/Subscriber] Drop", self.room);
        assert_eq!(self.queue.len(), 0, "Queue not empty on drop {:?}", self.queue);
        assert_eq!(self.channels.len(), 0, "Channels not empty on drop {:?}", self.channels);
        assert_eq!(self.subscribers.len(), 0, "Subscribers not empty on drop {:?}", self.subscribers);
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
        cluster::{ClusterEndpointEvent, ClusterLocalTrackEvent},
        transport::LocalTrackId,
    };

    use super::id_generator::gen_track_channel_id;
    use super::{Output, RoomChannelSubscribe};
    use super::{BITRATE_FEEDBACK_INTERVAL, BITRATE_FEEDBACK_KIND, BITRATE_FEEDBACK_TIMEOUT, KEYFRAME_FEEDBACK_INTERVAL, KEYFRAME_FEEDBACK_KIND, KEYFRAME_FEEDBACK_TIMEOUT};

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
    #[test_log::test]
    fn normal_sub_ubsub() {
        let room = 1.into();
        let mut subscriber = RoomChannelSubscribe::<u8>::new(room);

        let endpoint = 2;
        let track = LocalTrackId::from(3);
        let target_peer: PeerId = "peer2".to_string().into();
        let target_track: TrackName = "audio_main".to_string().into();
        let channel_id = gen_track_channel_id(room, &target_peer, &target_track);
        subscriber.on_track_subscribe(endpoint, track, target_peer.clone(), target_track.clone());
        assert_eq!(subscriber.pop_output(()), Some(Output::Pubsub(Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(subscriber.pop_output(()), None);

        let pkt = fake_audio();
        subscriber.on_track_data(channel_id, pkt.serialize());
        assert_eq!(
            subscriber.pop_output(()),
            Some(Output::Endpoint(
                vec![endpoint],
                ClusterEndpointEvent::LocalTrack(track, ClusterLocalTrackEvent::Media(*channel_id, pkt))
            ))
        );
        assert_eq!(subscriber.pop_output(()), None);

        subscriber.on_track_unsubscribe(endpoint, track);
        assert_eq!(subscriber.pop_output(()), Some(Output::Pubsub(Control(channel_id, ChannelControl::UnsubAuto))));
        assert_eq!(subscriber.pop_output(()), None);
        assert!(subscriber.is_empty());
    }

    //TODO Sending key-frame request
    #[test_log::test]
    fn send_key_frame() {
        let room = 1.into();
        let mut subscriber = RoomChannelSubscribe::<u8>::new(room);

        let endpoint = 2;
        let track = LocalTrackId::from(3);
        let target_peer: PeerId = "peer2".to_string().into();
        let target_track: TrackName = "audio_main".to_string().into();
        let channel_id = gen_track_channel_id(room, &target_peer, &target_track);
        subscriber.on_track_subscribe(endpoint, track, target_peer.clone(), target_track.clone());
        assert_eq!(subscriber.pop_output(()), Some(Output::Pubsub(Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(subscriber.pop_output(()), None);

        subscriber.on_track_request_key(endpoint, track);
        assert_eq!(
            subscriber.pop_output(()),
            Some(Output::Pubsub(Control(
                channel_id,
                ChannelControl::FeedbackAuto(Feedback::simple(KEYFRAME_FEEDBACK_KIND, 1, KEYFRAME_FEEDBACK_INTERVAL, KEYFRAME_FEEDBACK_TIMEOUT))
            )))
        );
        assert_eq!(subscriber.pop_output(()), None);

        subscriber.on_track_unsubscribe(endpoint, track);
        assert_eq!(subscriber.pop_output(()), Some(Output::Pubsub(Control(channel_id, ChannelControl::UnsubAuto))));
        assert_eq!(subscriber.pop_output(()), None);
        assert!(subscriber.is_empty());
    }

    //TODO Sending bitrate request single sub
    #[test_log::test]
    fn send_bitrate_limit_speed() {
        let room = 1.into();
        let mut subscriber = RoomChannelSubscribe::<u8>::new(room);

        let endpoint1 = 2;
        let track1 = LocalTrackId::from(3);
        let target_peer: PeerId = "peer2".to_string().into();
        let target_track: TrackName = "audio_main".to_string().into();
        let channel_id = gen_track_channel_id(room, &target_peer, &target_track);
        subscriber.on_track_subscribe(endpoint1, track1, target_peer.clone(), target_track.clone());
        assert_eq!(subscriber.pop_output(()), Some(Output::Pubsub(Control(channel_id, ChannelControl::SubAuto))));
        assert_eq!(subscriber.pop_output(()), None);

        let mut now = Instant::now();

        subscriber.on_track_desired_bitrate(now, endpoint1, track1, 1000);
        assert_eq!(
            subscriber.pop_output(()),
            Some(Output::Pubsub(Control(
                channel_id,
                ChannelControl::FeedbackAuto(Feedback::simple(BITRATE_FEEDBACK_KIND, 1000, BITRATE_FEEDBACK_INTERVAL, BITRATE_FEEDBACK_TIMEOUT))
            )))
        );
        assert_eq!(subscriber.pop_output(()), None);

        // more local track sub that channel
        let endpoint2 = 3;
        let track2 = LocalTrackId::from(4);
        subscriber.on_track_subscribe(endpoint2, track2, target_peer.clone(), target_track.clone());
        assert_eq!(subscriber.pop_output(()), None);

        // more feedback from local track2
        now += Duration::from_millis(100);
        subscriber.on_track_desired_bitrate(now, endpoint2, track2, 2000);
        assert_eq!(
            subscriber.pop_output(()),
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
        assert_eq!(subscriber.pop_output(()), None);

        //now last update from track2 after long time cause track1 feedback will be timeout
        now += Duration::from_millis(BITRATE_FEEDBACK_TIMEOUT as u64 - 100);
        subscriber.on_track_desired_bitrate(now, endpoint2, track2, 3000);
        assert_eq!(
            subscriber.pop_output(()),
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
        assert_eq!(subscriber.pop_output(()), None);

        subscriber.on_track_unsubscribe(endpoint1, track1);
        subscriber.on_track_unsubscribe(endpoint2, track2);
        assert_eq!(subscriber.pop_output(()), Some(Output::Pubsub(Control(channel_id, ChannelControl::UnsubAuto))));
        assert_eq!(subscriber.pop_output(()), None);
        assert!(subscriber.is_empty());
    }
}
