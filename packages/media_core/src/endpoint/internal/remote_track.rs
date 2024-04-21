use std::time::Instant;

use crate::transport::{RemoteTrackEvent, RemoteTrackId};

pub enum Output {}

#[derive(Default)]
pub struct EndpointRemoteTrack {}

impl EndpointRemoteTrack {
    pub fn on_connected(&mut self, now: Instant) -> Option<Output> {
        None
    }
    pub fn on_event(&mut self, now: Instant, event: RemoteTrackEvent) -> Option<Output> {
        None
    }
    pub fn pop_output(&mut self) -> Option<Output> {
        None
    }
}
