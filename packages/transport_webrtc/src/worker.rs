use std::time::Instant;

use media_server_core::endpoint::{Endpoint, Input, Output};
use sans_io_runtime::{group_task, TaskSwitcher};

use crate::{shared_port::SharedUdpPort, transport::TransportWebrtc};

group_task!(Endpoints, Endpoint<TransportWebrtc>, Input<'a>, Output<'a>);

pub struct MediaWorkerWebrtc {
    udp: SharedUdpPort<usize>,
    endpoints: Endpoints,
}

impl MediaWorkerWebrtc {
    pub fn new() -> Self {
        Self {
            udp: SharedUdpPort::default(),
            endpoints: Endpoints::default(),
        }
    }

    pub fn on_tick<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        todo!()
    }

    pub fn on_event<'a>(&mut self, now: Instant, input: Input<'a>) -> Option<Output<'a>> {
        todo!()
    }

    pub fn pop_output<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        todo!()
    }

    pub fn shutdown<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        todo!()
    }
}
