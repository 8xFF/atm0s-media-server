use std::{marker::PhantomData, time::Instant};

use atm0s_sdn::features::{FeaturesControl, FeaturesEvent};
use media_server_protocol::{
    endpoint::{PeerId, RoomId, TrackMeta, TrackName},
    media::MediaPacket,
};

use crate::transport::{LocalTrackId, RemoteTrackId};

#[derive(Clone)]
pub enum ClusterRemoteTrackControl {
    Started(TrackName),
    Media(MediaPacket),
    Ended,
}

#[derive(Clone)]
pub enum ClusterRemoteTrackEvent {
    RequestKeyFrame,
}

#[derive(Clone)]
pub enum ClusterLocalTrackControl {
    Subscribe(PeerId, TrackName),
    RequestKeyFrame,
    Unsubscribe,
}

#[derive(Clone)]
pub enum ClusterLocalTrackEvent {
    Started,
    Media(MediaPacket),
    Ended,
}

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

#[derive(Debug)]
pub struct MediaCluster<Owner> {
    _tmp: PhantomData<Owner>,
}

impl<Owner> Default for MediaCluster<Owner> {
    fn default() -> Self {
        Self { _tmp: PhantomData }
    }
}

impl<Owner> MediaCluster<Owner> {
    pub fn on_tick(&mut self, now: Instant) -> Option<Output<Owner>> {
        //TODO
        None
    }

    pub fn on_input(&mut self, now: Instant, input: Input<Owner>) -> Option<Output<Owner>> {
        //TODO
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
