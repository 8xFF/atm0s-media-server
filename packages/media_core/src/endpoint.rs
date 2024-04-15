use std::time::Instant;

use sans_io_runtime::backend::{BackendIncoming, BackendOutgoing};

use crate::{
    cluster::{EndpointControl, EndpointEvent},
    transport::{Transport, TransportInput, TransportOutput},
};

mod middleware;

pub struct EndpointSession(pub u64);

pub enum Input<'a> {
    Net(BackendIncoming<'a>),
    Sdn(EndpointEvent),
}

pub enum Output<'a> {
    Net(BackendOutgoing<'a>),
    Sdn(EndpointControl),
}

pub struct Endpoint<T: Transport> {
    transport: T,
    middlewares: Vec<Box<dyn middleware::EndpointMiddleware>>,
}

impl<T: Transport> Endpoint<T> {
    pub fn on_tick<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        let out = self.transport.on_tick(now)?;
        self.process_transport_output(out)
    }

    pub fn on_event<'a>(&mut self, now: Instant, input: Input<'a>) -> Option<Output<'a>> {
        let input = match input {
            Input::Net(net) => TransportInput::Net(net),
            _ => todo!(),
        };
        let out = self.transport.on_input(now, input)?;
        self.process_transport_output(out)
    }

    pub fn pop_output<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        let out = self.transport.pop_output(now)?;
        self.process_transport_output(out)
    }

    pub fn shutdown<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        todo!()
    }
}

impl<T: Transport> Endpoint<T> {
    fn process_transport_output<'a>(&mut self, out: TransportOutput<'a>) -> Option<Output<'a>> {
        todo!()
    }
}
