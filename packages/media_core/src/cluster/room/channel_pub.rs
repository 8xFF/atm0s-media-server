//! Channel Publisher will takecare of pubsub channel for sending data and handle when received channel feedback

use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
    time::Instant,
};

use atm0s_sdn::features::pubsub::{self, ChannelControl, ChannelId, Feedback};
use media_server_protocol::{
    endpoint::{PeerId, TrackName},
    media::MediaPacket,
};
use sans_io_runtime::{return_if_err, return_if_none, TaskSwitcherChild};

use crate::{
    cluster::{id_generator, ClusterEndpointEvent, ClusterRemoteTrackEvent, ClusterRoomHash},
    transport::RemoteTrackId,
};

pub enum FeedbackKind {
    Bitrate { min: u64, max: u64 },
    KeyFrameRequest,
}

impl TryFrom<Feedback> for FeedbackKind {
    type Error = ();
    fn try_from(value: Feedback) -> Result<Self, Self::Error> {
        match value.kind {
            0 => Ok(FeedbackKind::Bitrate { min: value.min, max: value.max }),
            1 => Ok(FeedbackKind::KeyFrameRequest),
            _ => Err(()),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Output<Owner> {
    Endpoint(Vec<Owner>, ClusterEndpointEvent),
    Pubsub(pubsub::Control),
}

pub struct RoomChannelPublisher<Owner> {
    room: ClusterRoomHash,
    tracks: HashMap<(Owner, RemoteTrackId), (PeerId, TrackName, ChannelId)>,
    tracks_source: HashMap<ChannelId, (Owner, RemoteTrackId)>,
    queue: VecDeque<Output<Owner>>,
}

impl<Owner: Debug + Hash + Eq + Copy> RoomChannelPublisher<Owner> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            tracks: HashMap::new(),
            tracks_source: HashMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn on_channel_feedback(&mut self, channel: ChannelId, fb: Feedback) {
        let fb = return_if_err!(FeedbackKind::try_from(fb));
        let (owner, track_id) = return_if_none!(self.tracks_source.get(&channel));
        match fb {
            FeedbackKind::Bitrate { min, max } => {
                log::debug!("[ClusterRoom {}/Publishers] channel {channel} limit bitrate [{min},{max}]", self.room);
                self.queue.push_back(Output::Endpoint(
                    vec![*owner],
                    ClusterEndpointEvent::RemoteTrack(*track_id, ClusterRemoteTrackEvent::LimitBitrate { min, max }),
                ));
            }
            FeedbackKind::KeyFrameRequest => {
                log::debug!("[ClusterRoom {}/Publishers] channel {channel} request key_frame", self.room);
                self.queue
                    .push_back(Output::Endpoint(vec![*owner], ClusterEndpointEvent::RemoteTrack(*track_id, ClusterRemoteTrackEvent::RequestKeyFrame)));
            }
        }
    }

    pub fn on_track_publish(&mut self, owner: Owner, track: RemoteTrackId, peer: PeerId, name: TrackName) {
        log::info!("[ClusterRoom {}/Publishers] peer ({peer} started track ({name})", self.room);
        let channel_id = id_generator::gen_channel_id(self.room, &peer, &name);
        self.tracks.insert((owner, track), (peer.clone(), name.clone(), channel_id));
        self.tracks_source.insert(channel_id, (owner, track));

        self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart)));
    }

    pub fn on_track_data(&mut self, owner: Owner, track: RemoteTrackId, media: MediaPacket) {
        log::trace!(
            "[ClusterRoom {}/Publishers] peer {:?} track {track} publish media meta {:?} seq {}",
            self.room,
            owner,
            media.meta,
            media.seq
        );
        let (_peer, _name, channel_id) = return_if_none!(self.tracks.get(&(owner, track)));
        let data = media.serialize();
        self.queue.push_back(Output::Pubsub(pubsub::Control(*channel_id, ChannelControl::PubData(data))))
    }

    pub fn on_track_unpublish(&mut self, owner: Owner, track: RemoteTrackId) {
        let (peer, name, channel_id) = return_if_none!(self.tracks.remove(&(owner, track)));
        self.tracks_source.remove(&channel_id).expect("Should have track_source");
        log::info!("[ClusterRoom {}/Publishers] peer ({peer} stopped track {name})", self.room);
        self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop)))
    }
}

impl<Owner: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Owner>> for RoomChannelPublisher<Owner> {
    type Time = Instant;
    fn pop_output(&mut self, _now: Instant) -> Option<Output<Owner>> {
        self.queue.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use atm0s_sdn::features::pubsub::{ChannelControl, Control, Feedback};
    use media_server_protocol::media::{MediaMeta, MediaPacket};
    use sans_io_runtime::TaskSwitcherChild;

    use crate::{
        cluster::{ClusterEndpointEvent, ClusterRemoteTrackEvent},
        transport::RemoteTrackId,
    };

    use super::id_generator::gen_channel_id;
    use super::{Output, RoomChannelPublisher};

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

    //Track start => should register with SDN
    //Track stop => should unregister with SDN
    //Track media => should send data over SDN
    #[test]
    fn channel_publish_data() {
        let room = 1.into();
        let mut publisher = RoomChannelPublisher::<u8>::new(room);

        let owner = 2;
        let track = RemoteTrackId(3);
        let peer = "peer1".to_string().into();
        let name = "audio_main".to_string().into();
        let channel_id = gen_channel_id(room, &peer, &name);
        publisher.on_track_publish(owner, track, peer, name);
        assert_eq!(publisher.pop_output(Instant::now()), Some(Output::Pubsub(Control(channel_id, ChannelControl::PubStart))));
        assert_eq!(publisher.pop_output(Instant::now()), None);

        let media = fake_audio();
        publisher.on_track_data(owner, track, media.clone());
        assert_eq!(
            publisher.pop_output(Instant::now()),
            Some(Output::Pubsub(Control(channel_id, ChannelControl::PubData(media.serialize()))))
        );
        assert_eq!(publisher.pop_output(Instant::now()), None);

        publisher.on_track_unpublish(owner, track);
        assert_eq!(publisher.pop_output(Instant::now()), Some(Output::Pubsub(Control(channel_id, ChannelControl::PubStop))));
        assert_eq!(publisher.pop_output(Instant::now()), None);
    }

    //TODO Handle feedback: should handle KeyFrame feedback
    //TODO Handle feedback: should handle Bitrate feedback
    #[test]
    fn channel_feedback() {
        let room = 1.into();
        let mut publisher = RoomChannelPublisher::<u8>::new(room);

        let owner = 2;
        let track = RemoteTrackId(3);
        let peer = "peer1".to_string().into();
        let name = "audio_main".to_string().into();
        let channel_id = gen_channel_id(room, &peer, &name);
        publisher.on_track_publish(owner, track, peer, name);
        assert_eq!(publisher.pop_output(Instant::now()), Some(Output::Pubsub(Control(channel_id, ChannelControl::PubStart))));
        assert_eq!(publisher.pop_output(Instant::now()), None);

        publisher.on_channel_feedback(channel_id, Feedback::simple(0, 1000, 100, 200));
        assert_eq!(
            publisher.pop_output(Instant::now()),
            Some(Output::Endpoint(
                vec![owner],
                ClusterEndpointEvent::RemoteTrack(track, ClusterRemoteTrackEvent::LimitBitrate { min: 1000, max: 1000 })
            ))
        );

        publisher.on_channel_feedback(channel_id, Feedback::simple(1, 1, 100, 200));
        assert_eq!(
            publisher.pop_output(Instant::now()),
            Some(Output::Endpoint(vec![owner], ClusterEndpointEvent::RemoteTrack(track, ClusterRemoteTrackEvent::RequestKeyFrame)))
        );
    }
}
