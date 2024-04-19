use std::time::Instant;

use media_server_protocol::{
    endpoint::{PeerId, TrackName},
    media::MediaPacket,
};
use media_server_utils::F16u;
use sans_io_runtime::backend::{BackendIncoming, BackendOutgoing};

use crate::endpoint::{EndpointEvent, EndpointReq, EndpointReqId, EndpointRes};

#[derive(Clone, Copy)]
pub struct TransportId(pub u64);

/// RemoteTrackId is used for track which received media from client
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RemoteTrackId(pub u16);

/// LocalTrackId is used for track which send media to client
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LocalTrackId(pub u16);

pub enum TransportError {
    Timeout,
}

pub enum TransportState {
    Connecting,
    ConnectError(TransportError),
    Connected,
    Reconnecting,
    Disconnected(Option<TransportError>),
}

pub struct TransportStats {
    pub sent_bytes: u64,
    pub recv_bytes: u64,
    pub sent_loss: F16u,
    pub recv_loss: F16u,
}

/// This is used for notifying state of local track to endpoint
pub enum LocalTrackEvent {
    Started,
    Switch(Option<(PeerId, TrackName)>),
    RequestKeyFrame,
    Ended,
}

/// This is used for notifying state of remote track to endpoint
pub enum RemoteTrackEvent {
    Started { name: String },
    Paused,
    Resumed,
    Media(MediaPacket),
    Ended,
}

pub enum TransportEvent {
    State(TransportState),
    RemoteTrack(RemoteTrackId, RemoteTrackEvent),
    LocalTrack(LocalTrackId, LocalTrackEvent),
    Stats(TransportStats),
}

/// This is control message from endpoint
pub enum TransportInput<'a, Ext> {
    Net(BackendIncoming<'a>),
    Endpoint(EndpointEvent),
    RpcRes(EndpointReqId, EndpointRes),
    Ext(Ext),
    Close,
}

/// This is event from transport, in general is is result of transport protocol
pub enum TransportOutput<'a, Ext> {
    Net(BackendOutgoing<'a>),
    Event(TransportEvent),
    RpcReq(EndpointReqId, EndpointReq),
    Ext(Ext),
}

pub trait Transport<ExtIn, ExtOut> {
    fn on_tick<'a>(&mut self, now: Instant) -> Option<TransportOutput<'a, ExtOut>>;
    fn on_input<'a>(&mut self, now: Instant, input: TransportInput<'a, ExtIn>) -> Option<TransportOutput<'a, ExtOut>>;
    fn pop_event<'a>(&mut self, now: Instant) -> Option<TransportOutput<'a, ExtOut>>;
}
