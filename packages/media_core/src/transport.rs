use std::time::Instant;

use media_server_protocol::media::MediaPacket;
use sans_io_runtime::backend::{BackendIncoming, BackendOutgoing};

pub struct TransportSession(pub u64);
pub struct TrackId(pub u16);

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

pub struct TransportStats {}

pub enum TransportControlIn {}

pub enum TransportControlOut {}

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

pub enum TransportInput<'a> {
    Net(BackendIncoming<'a>),
    RemoteMediaTrack(u16, RemoteTrackControl),
    LocalMediaTrack(u16, LocalTrackControl),
    Control(TransportControlIn),
}

pub enum TransportOutput<'a> {
    Net(BackendOutgoing<'a>),
    State(TransportState),
    RemoteTrack(u16, RemoteTrackEvent),
    LocalTrack(u16, LocalTrackEvent),
    Stats(TransportStats),
    Control(TransportControlOut),
}

pub trait Transport {
    fn on_tick<'a>(&mut self, now: Instant) -> Option<TransportOutput<'a>>;
    fn on_input<'a>(&mut self, now: Instant, input: TransportInput<'a>) -> Option<TransportOutput<'a>>;
    fn pop_output<'a>(&mut self, now: Instant) -> Option<TransportOutput<'a>>;
}
