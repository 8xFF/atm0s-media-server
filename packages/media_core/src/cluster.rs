use std::{fmt::Debug, hash::Hash, time::Instant};

use atm0s_sdn::features::{FeaturesControl, FeaturesEvent};
use media_server_protocol::{
    endpoint::{PeerId, RoomId, TrackMeta, TrackName},
    media::MediaPacket,
};
use media_server_utils::Small2dMap;

use crate::transport::{LocalTrackId, RemoteTrackId};

mod room;

#[derive(Debug, Clone)]
pub enum ClusterRemoteTrackControl {
    Started(TrackName),
    Media(MediaPacket),
    Ended,
}

#[derive(Clone)]
pub enum ClusterRemoteTrackEvent {
    RequestKeyFrame,
}

#[derive(Debug, Clone)]
pub enum ClusterLocalTrackControl {
    Subscribe(PeerId, TrackName),
    RequestKeyFrame,
    Unsubscribe,
}

#[derive(Debug, Clone)]
pub enum ClusterLocalTrackEvent {
    Started,
    Media(MediaPacket),
    Ended,
}

#[derive(Debug)]
pub enum ClusterEndpointControl {
    JoinRoom(RoomId, PeerId),
    LeaveRoom,
    SubscribeRoom,
    UnsubscribeRoom,
    SubscribePeer(PeerId),
    UnsubscribePeer(PeerId),
    RemoteTrack(RemoteTrackId, ClusterRemoteTrackControl),
    LocalTrack(LocalTrackId, ClusterLocalTrackControl),
}

#[derive(Clone)]
pub enum ClusterEndpointEvent {
    PeerJoined(PeerId),
    PeerLeaved(PeerId),
    TrackStarted(PeerId, TrackName, TrackMeta),
    TrackStoped(PeerId, TrackName),
    RemoteTrack(RemoteTrackId, ClusterRemoteTrackEvent),
    LocalTrack(LocalTrackId, ClusterLocalTrackEvent),
}

pub enum Input<Owner> {
    Sdn(FeaturesEvent),
    Endpoint(Owner, ClusterEndpointControl),
}

pub enum Output<Owner> {
    Sdn(FeaturesControl),
    Endpoint(Vec<Owner>, ClusterEndpointEvent),
}

pub struct MediaCluster<Owner> {
    endpoints: Small2dMap<Owner, RoomId>,
}

impl<Owner: Hash + Eq + Clone> Default for MediaCluster<Owner> {
    fn default() -> Self {
        Self { endpoints: Small2dMap::default() }
    }
}

impl<Owner: Debug> MediaCluster<Owner> {
    pub fn on_tick(&mut self, now: Instant) -> Option<Output<Owner>> {
        //TODO
        None
    }

    pub fn on_sdn_event(&mut self, now: Instant, event: FeaturesEvent) -> Option<Output<Owner>> {
        None
    }

    pub fn on_endpoint_control(&mut self, now: Instant, owner: Owner, control: ClusterEndpointControl) -> Option<Output<Owner>> {
        log::info!("[MediaCluster] {:?} control {:?}", owner, control);
        None
    }

    pub fn pop_output(&mut self, now: Instant) -> Option<Output<Owner>> {
        //TODO
        None
    }

    pub fn shutdown<'a>(&mut self, now: Instant) -> Option<Output<Owner>> {
        //TODO
        None
    }
}
