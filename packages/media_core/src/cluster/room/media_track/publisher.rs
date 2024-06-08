//!
//! Channel Publisher will takecare of pubsub channel for sending data and handle when received channel feedback
//!

use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
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

use super::Output;

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

pub struct RoomChannelPublisher<Endpoint> {
    room: ClusterRoomHash,
    tracks: HashMap<(Endpoint, RemoteTrackId), (PeerId, TrackName, ChannelId)>,
    tracks_source: HashMap<ChannelId, (Endpoint, RemoteTrackId)>,
    queue: VecDeque<Output<Endpoint>>,
}

impl<Endpoint: Debug + Hash + Eq + Copy> RoomChannelPublisher<Endpoint> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            tracks: HashMap::new(),
            tracks_source: HashMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn on_track_feedback(&mut self, channel: ChannelId, fb: Feedback) {
        let fb = return_if_err!(FeedbackKind::try_from(fb));
        let (endpoint, track_id) = return_if_none!(self.tracks_source.get(&channel));
        match fb {
            FeedbackKind::Bitrate { min, max } => {
                log::debug!("[ClusterRoom {}/Publishers] channel {channel} limit bitrate [{min},{max}]", self.room);
                self.queue.push_back(Output::Endpoint(
                    vec![*endpoint],
                    ClusterEndpointEvent::RemoteTrack(*track_id, ClusterRemoteTrackEvent::LimitBitrate { min, max }),
                ));
            }
            FeedbackKind::KeyFrameRequest => {
                log::debug!("[ClusterRoom {}/Publishers] channel {channel} request key_frame", self.room);
                self.queue.push_back(Output::Endpoint(
                    vec![*endpoint],
                    ClusterEndpointEvent::RemoteTrack(*track_id, ClusterRemoteTrackEvent::RequestKeyFrame),
                ));
            }
        }
    }

    pub fn on_track_publish(&mut self, endpoint: Endpoint, track: RemoteTrackId, peer: PeerId, name: TrackName) {
        log::info!("[ClusterRoom {}/Publishers] peer ({peer} started track ({name})", self.room);
        let channel_id = id_generator::gen_channel_id(self.room, &peer, &name);
        self.tracks.insert((endpoint, track), (peer.clone(), name.clone(), channel_id));
        self.tracks_source.insert(channel_id, (endpoint, track));

        self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart)));
    }

    pub fn on_track_data(&mut self, endpoint: Endpoint, track: RemoteTrackId, media: MediaPacket) {
        log::trace!(
            "[ClusterRoom {}/Publishers] peer {:?} track {track} publish media meta {:?} seq {}",
            self.room,
            endpoint,
            media.meta,
            media.seq
        );
        let (_peer, _name, channel_id) = return_if_none!(self.tracks.get(&(endpoint, track)));
        let data = media.serialize();
        self.queue.push_back(Output::Pubsub(pubsub::Control(*channel_id, ChannelControl::PubData(data))))
    }

    pub fn on_track_unpublish(&mut self, endpoint: Endpoint, track: RemoteTrackId) {
        let (peer, name, channel_id) = return_if_none!(self.tracks.remove(&(endpoint, track)));
        self.tracks_source.remove(&channel_id).expect("Should have track_source");
        log::info!("[ClusterRoom {}/Publishers] peer ({peer} stopped track {name})", self.room);
        self.queue.push_back(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop)));
        if self.tracks.is_empty() {
            self.queue.push_back(Output::OnResourceEmpty);
        }
    }
}

impl<Endpoint: Debug + Hash + Eq + Copy> TaskSwitcherChild<Output<Endpoint>> for RoomChannelPublisher<Endpoint> {
    type Time = ();
    fn pop_output(&mut self, _now: Self::Time) -> Option<Output<Endpoint>> {
        self.queue.pop_front()
    }
}

impl<Endpoint> Drop for RoomChannelPublisher<Endpoint> {
    fn drop(&mut self) {
        log::info!("[ClusterRoom {}/Publishers] Drop", self.room);
        assert_eq!(self.queue.len(), 0, "Queue not empty on drop");
        assert_eq!(self.tracks.len(), 0, "Tracks not empty on drop");
        assert_eq!(self.tracks_source.len(), 0, "Tracks source not empty on drop");
    }
}

#[cfg(test)]
mod tests {
    use atm0s_sdn::features::pubsub::{ChannelControl, Control, Feedback};
    use media_server_protocol::media::{MediaMeta, MediaPacket};
    use sans_io_runtime::TaskSwitcherChild;

    use crate::{
        cluster::{ClusterEndpointEvent, ClusterRemoteTrackEvent},
        transport::RemoteTrackId,
    };

    use super::id_generator::gen_channel_id;
    use super::{super::Output, RoomChannelPublisher};

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

        let endpoint = 2;
        let track = RemoteTrackId(3);
        let peer = "peer1".to_string().into();
        let name = "audio_main".to_string().into();
        let channel_id = gen_channel_id(room, &peer, &name);
        publisher.on_track_publish(endpoint, track, peer, name);
        assert_eq!(publisher.pop_output(()), Some(Output::Pubsub(Control(channel_id, ChannelControl::PubStart))));
        assert_eq!(publisher.pop_output(()), None);

        let media = fake_audio();
        publisher.on_track_data(endpoint, track, media.clone());
        assert_eq!(publisher.pop_output(()), Some(Output::Pubsub(Control(channel_id, ChannelControl::PubData(media.serialize())))));
        assert_eq!(publisher.pop_output(()), None);

        publisher.on_track_unpublish(endpoint, track);
        assert_eq!(publisher.pop_output(()), Some(Output::Pubsub(Control(channel_id, ChannelControl::PubStop))));
        assert_eq!(publisher.pop_output(()), Some(Output::OnResourceEmpty));
        assert_eq!(publisher.pop_output(()), None);
    }

    //TODO Handle feedback: should handle KeyFrame feedback
    //TODO Handle feedback: should handle Bitrate feedback
    #[test]
    fn channel_feedback() {
        let room = 1.into();
        let mut publisher = RoomChannelPublisher::<u8>::new(room);

        let endpoint = 2;
        let track = RemoteTrackId(3);
        let peer = "peer1".to_string().into();
        let name = "audio_main".to_string().into();
        let channel_id = gen_channel_id(room, &peer, &name);
        publisher.on_track_publish(endpoint, track, peer, name);
        assert_eq!(publisher.pop_output(()), Some(Output::Pubsub(Control(channel_id, ChannelControl::PubStart))));
        assert_eq!(publisher.pop_output(()), None);

        publisher.on_track_feedback(channel_id, Feedback::simple(0, 1000, 100, 200));
        assert_eq!(
            publisher.pop_output(()),
            Some(Output::Endpoint(
                vec![endpoint],
                ClusterEndpointEvent::RemoteTrack(track, ClusterRemoteTrackEvent::LimitBitrate { min: 1000, max: 1000 })
            ))
        );

        publisher.on_track_feedback(channel_id, Feedback::simple(1, 1, 100, 200));
        assert_eq!(
            publisher.pop_output(()),
            Some(Output::Endpoint(vec![endpoint], ClusterEndpointEvent::RemoteTrack(track, ClusterRemoteTrackEvent::RequestKeyFrame)))
        );
        assert_eq!(publisher.pop_output(()), None);

        publisher.on_track_unpublish(endpoint, track);
        assert_eq!(publisher.pop_output(()), Some(Output::Pubsub(Control(channel_id, ChannelControl::PubStop))));
        assert_eq!(publisher.pop_output(()), Some(Output::OnResourceEmpty));
        assert_eq!(publisher.pop_output(()), None);
    }
}
