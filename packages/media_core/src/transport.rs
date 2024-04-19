use std::time::Instant;

use media_server_protocol::{
    endpoint::{PeerId, RoomId, TrackMeta, TrackName},
    media::MediaPacket,
};
use media_server_utils::F16u;
use sans_io_runtime::backend::{BackendIncoming, BackendOutgoing};

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

pub enum ClientRemoteTrackControl {
    Started(TrackName),
    Media(MediaPacket),
    Ended,
}

pub enum ClientRemoteTrackEvent {
    RequestKeyFrame,
    LimitBitrateBps(u64),
}

pub enum ClientLocalTrackControl {
    Subscribe(PeerId, TrackName),
    RequestKeyFrame,
    Unsubscribe,
}

pub enum ClientLocalTrackEvent {
    Started,
    Media(MediaPacket),
    Ended,
}

pub enum ClientEndpointControl {
    JoinRoom(RoomId, PeerId),
    LeaveRoom,
    RemoteTrack(RemoteTrackId, ClientRemoteTrackControl),
    LocalTrack(LocalTrackId, ClientLocalTrackControl),
}

pub enum ClientEndpointEvent {
    PeerJoined(PeerId),
    PeerLeaved(PeerId),
    PeerTrackStarted(PeerId, TrackName, TrackMeta),
    PeerTrackStopped(PeerId, TrackName),
    RemoteTrack(RemoteTrackId, ClientRemoteTrackEvent),
    LocalTrack(LocalTrackId, ClientLocalTrackEvent),
}

pub enum LocalTrackControl {
    Media(MediaPacket),
}

pub enum LocalTrackEvent {
    Started { name: String },
    Paused,
    RequestKeyFrame,
    Ended,
}

pub enum RemoteTrackControl {
    RequestKeyFrame,
}

pub enum RemoteTrackEvent {
    Started { name: String },
    Paused,
    Media(MediaPacket),
    Ended,
}

pub enum TransportControl<'a, Ext> {
    Net(BackendIncoming<'a>),
    RemoteMediaTrack(RemoteTrackId, RemoteTrackControl),
    LocalMediaTrack(LocalTrackId, LocalTrackControl),
    Event(ClientEndpointEvent),
    Ext(Ext),
    Close,
}

pub enum TransportEvent<'a, Ext> {
    Net(BackendOutgoing<'a>),
    State(TransportState),
    RemoteTrack(RemoteTrackId, RemoteTrackEvent),
    LocalTrack(LocalTrackId, LocalTrackEvent),
    Stats(TransportStats),
    Control(ClientEndpointControl),
    Ext(Ext),
}

pub trait Transport<ExtIn, ExtOut> {
    fn on_tick<'a>(&mut self, now: Instant) -> Option<TransportEvent<'a, ExtOut>>;
    fn on_control<'a>(&mut self, now: Instant, input: TransportControl<'a, ExtIn>) -> Option<TransportEvent<'a, ExtOut>>;
    fn pop_event<'a>(&mut self, now: Instant) -> Option<TransportEvent<'a, ExtOut>>;
}
