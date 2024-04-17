use std::{marker::PhantomData, time::Instant};

use atm0s_sdn::features::{FeaturesControl, FeaturesEvent};

pub enum EndpointControl {}

#[derive(Clone)]
pub enum EndpointEvent {
    Test,
}

pub enum Input<Owner> {
    Sdn(FeaturesEvent),
    Endpoint(Owner, EndpointControl),
}

pub enum Output<Owner> {
    Sdn(FeaturesControl),
    Endpoint(Vec<Owner>, EndpointEvent),
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
        todo!()
    }

    pub fn on_input(&mut self, now: Instant, input: Input<Owner>) -> Option<Output<Owner>> {
        todo!()
    }

    pub fn pop_output(&mut self, now: Instant) -> Option<Output<Owner>> {
        todo!()
    }

    pub fn shutdown<'a>(&mut self, now: Instant) -> Option<Output<Owner>> {
        todo!()
    }
}
