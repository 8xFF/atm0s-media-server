use std::time::Instant;

use crate::transport::{LocalTrackEvent, LocalTrackId};

pub enum Output {}

#[derive(Default)]
pub struct EndpointLocalTrack {}

impl EndpointLocalTrack {
    pub fn on_connected(&mut self, now: Instant) -> Option<Output> {
        None
    }
    pub fn on_event(&mut self, now: Instant, event: LocalTrackEvent) -> Option<Output> {
        None
    }
    pub fn pop_output(&mut self) -> Option<Output> {
        None
    }
}
