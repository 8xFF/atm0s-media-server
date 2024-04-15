use std::time::Instant;

use atm0s_sdn::features::{FeaturesControl, FeaturesEvent};

pub enum EndpointControl {}

pub enum EndpointEvent {}

pub enum Input {
    Sdn(FeaturesEvent),
    Endpoint(EndpointControl),
}

pub enum Output {
    Sdn(FeaturesControl),
    Endpoint(EndpointEvent),
}

#[derive(Debug, Default)]
pub struct MediaCluster {}

impl MediaCluster {
    pub fn on_tick(&mut self, now: Instant) -> Option<Output> {
        todo!()
    }

    pub fn on_input(&mut self, now: Instant, input: Input) -> Option<Output> {
        todo!()
    }

    pub fn pop_output(&mut self, now: Instant) -> Option<Output> {
        todo!()
    }
}
