use derive_more::{Display, From};
use std::{hash::Hash, time::Instant};

use media_server_protocol::{
    endpoint::{TrackMeta, TrackPriority},
    media::{MediaKind, MediaPacket},
};
use media_server_utils::F16u;
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    TaskSwitcherChild,
};

use crate::endpoint::{EndpointEvent, EndpointReq, EndpointReqId, EndpointRes};

#[derive(From, Debug, Clone, Copy, PartialEq, Eq, Display)]
pub struct TransportId(pub u64);

/// RemoteTrackId is used for track which received media from client
#[derive(From, Debug, Clone, Copy, PartialEq, Eq, Display)]
pub struct RemoteTrackId(pub u16);

impl Hash for RemoteTrackId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// LocalTrackId is used for track which send media to client
#[derive(From, Debug, Clone, Copy, PartialEq, Eq, Display)]
pub struct LocalTrackId(pub u16);

impl Hash for LocalTrackId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TransportError {
    Timeout,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TransportState {
    Connecting,
    ConnectError(TransportError),
    Connected,
    Reconnecting,
    Disconnected(Option<TransportError>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct TransportStats {
    pub sent_bytes: u64,
    pub recv_bytes: u64,
    pub sent_loss: F16u,
    pub recv_loss: F16u,
}

/// This is used for notifying state of local track to endpoint
#[derive(Debug, PartialEq, Eq)]
pub enum LocalTrackEvent {
    Started(MediaKind),
    RequestKeyFrame,
    Ended,
}

impl LocalTrackEvent {
    pub fn need_create(&self) -> Option<MediaKind> {
        if let LocalTrackEvent::Started(kind) = self {
            Some(*kind)
        } else {
            None
        }
    }
}

/// This is used for notifying state of remote track to endpoint
#[derive(Debug, PartialEq, Eq)]
pub enum RemoteTrackEvent {
    Started { name: String, priority: TrackPriority, meta: TrackMeta },
    Paused,
    Resumed,
    Media(MediaPacket),
    Ended,
}

impl RemoteTrackEvent {
    pub fn need_create(&self) -> Option<TrackMeta> {
        if let RemoteTrackEvent::Started { meta, .. } = self {
            Some(meta.clone())
        } else {
            None
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TransportEvent {
    State(TransportState),
    RemoteTrack(RemoteTrackId, RemoteTrackEvent),
    LocalTrack(LocalTrackId, LocalTrackEvent),
    Stats(TransportStats),
    EgressBitrateEstimate(u64),
}

/// This is control message from endpoint
pub enum TransportInput<Ext> {
    Net(BackendIncoming),
    Endpoint(EndpointEvent),
    RpcRes(EndpointReqId, EndpointRes),
    Ext(Ext),
    Close,
}

/// This is event from transport, in general is is result of transport protocol
#[derive(Debug, PartialEq, Eq)]
pub enum TransportOutput<Ext> {
    Net(BackendOutgoing),
    Event(TransportEvent),
    RpcReq(EndpointReqId, EndpointReq),
    Ext(Ext),
}

pub trait Transport<ExtIn, ExtOut>: TaskSwitcherChild<TransportOutput<ExtOut>> {
    fn on_tick(&mut self, now: Instant);
    fn on_input(&mut self, now: Instant, input: TransportInput<ExtIn>);
}
