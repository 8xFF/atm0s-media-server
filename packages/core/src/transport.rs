use std::time::Instant;

use sans_io_runtime::backend::{BackendIncoming, BackendOutgoing};

use crate::base::{LocalTrackControl, LocalTrackEvent, RemoteTrackControl, RemoteTrackEvent};

pub struct TransportSession(pub u64);
pub struct TrackId(pub u16);

pub enum TransportState {}

pub struct TransportStats {}

pub enum TransportControlIn {}

pub enum TransportControlOut {}

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
