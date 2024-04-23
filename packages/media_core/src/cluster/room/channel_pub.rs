use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
};

use atm0s_sdn::features::pubsub::{self, ChannelControl, ChannelId, Feedback};
use media_server_protocol::{
    endpoint::{PeerId, TrackMeta, TrackName},
    media::MediaPacket,
};

use crate::{
    cluster::{ClusterEndpointEvent, ClusterRemoteTrackEvent, ClusterRoomHash},
    transport::RemoteTrackId,
};

use super::FeedbackKind;

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

impl<Owner: Hash + Eq + Copy> RoomChannelPublisher<Owner> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            tracks: HashMap::new(),
            tracks_source: HashMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn on_channel_feedback(&mut self, channel: ChannelId, fb: Feedback) -> Option<Output<Owner>> {
        let fb_kind = FeedbackKind::try_from(fb.kind).ok()?;
        let (owner, track_id) = self.tracks_source.get(&channel)?;
        match fb_kind {
            FeedbackKind::Bitrate => todo!(),
            FeedbackKind::KeyFrameRequest => Some(Output::Endpoint(
                vec![owner.clone()],
                ClusterEndpointEvent::RemoteTrack(*track_id, ClusterRemoteTrackEvent::RequestKeyFrame),
            )),
        }
    }

    pub fn on_track_publish(&mut self, owner: Owner, track: RemoteTrackId, peer: PeerId, name: TrackName, meta: TrackMeta) -> Option<Output<Owner>> {
        log::info!("[ClusterRoom {}] peer ({peer} started track {name})", self.room);
        let channel_id = super::track_key(self.room, &peer, &name);
        self.tracks.insert((owner, track), (peer.clone(), name.clone(), channel_id));
        self.tracks_source.insert(channel_id, (owner, track));

        Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart)))
    }

    pub fn on_track_data(&mut self, owner: Owner, track: RemoteTrackId, media: MediaPacket) -> Option<Output<Owner>> {
        let (_peer, _name, channel_id) = self.tracks.get(&(owner, track))?;
        let data = media.serialize();
        Some(Output::Pubsub(pubsub::Control(*channel_id, ChannelControl::PubData(data))))
    }

    pub fn on_track_unpublish(&mut self, owner: Owner, track: RemoteTrackId) -> Option<Output<Owner>> {
        let (peer, name, channel_id) = self.tracks.remove(&(owner, track))?;
        self.tracks_source.remove(&channel_id).expect("Should have track_source");
        log::info!("[ClusterRoom {}] peer ({peer} stopped track {name})", self.room);
        Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop)))
    }

    pub fn pop_output(&mut self) -> Option<Output<Owner>> {
        self.queue.pop_front()
    }
}
