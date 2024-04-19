use std::{marker::PhantomData, time::Instant};

use sans_io_runtime::backend::{BackendIncoming, BackendOutgoing};

use crate::{
    cluster::{ClusterEndpointControl, ClusterEndpointEvent},
    transport::{Transport, TransportControl, TransportEvent},
};

use internal::EndpointInternal;

mod internal;
mod middleware;

pub struct EndpointSession(pub u64);

pub enum Input<'a, Ext> {
    Net(BackendIncoming<'a>),
    Cluster(ClusterEndpointEvent),
    Ext(Ext),
    Close,
}

pub enum Output<'a, Ext> {
    Net(BackendOutgoing<'a>),
    Cluster(ClusterEndpointControl),
    Ext(Ext),
    Shutdown,
}

pub struct Endpoint<T: Transport<ExtIn, ExtOut>, ExtIn, ExtOut> {
    transport: T,
    internal: EndpointInternal,
    _tmp: PhantomData<(ExtIn, ExtOut)>,
}

impl<T: Transport<ExtIn, ExtOut>, ExtIn, ExtOut> Endpoint<T, ExtIn, ExtOut> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            internal: EndpointInternal::new(),
            _tmp: PhantomData::default(),
        }
    }

    pub fn on_tick<'a>(&mut self, now: Instant) -> Option<Output<'a, ExtOut>> {
        if let Some(out) = self.internal.on_tick(now) {
            return self.process_internal_output(now, out);
        }
        let out = self.transport.on_tick(now)?;
        self.process_transport_output(now, out)
    }

    pub fn on_event<'a>(&mut self, now: Instant, input: Input<'a, ExtIn>) -> Option<Output<'a, ExtOut>> {
        match input {
            Input::Net(net) => {
                let out = self.transport.on_control(now, TransportControl::Net(net))?;
                self.process_transport_output(now, out)
            }
            Input::Ext(ext) => {
                let out = self.transport.on_control(now, TransportControl::Ext(ext))?;
                self.process_transport_output(now, out)
            }
            Input::Cluster(event) => {
                let out = self.internal.on_cluster_event(now, event)?;
                self.process_internal_output(now, out)
            }
            Input::Close => todo!(),
        }
    }

    pub fn pop_output<'a>(&mut self, now: Instant) -> Option<Output<'a, ExtOut>> {
        let out = self.transport.pop_event(now)?;
        self.process_transport_output(now, out)
    }

    pub fn shutdown<'a>(&mut self, now: Instant) -> Option<Output<'a, ExtOut>> {
        let out = self.internal.shutdown(now)?;
        self.process_internal_output(now, out)
    }
}

impl<T: Transport<ExtIn, ExtOut>, ExtIn, ExtOut> Endpoint<T, ExtIn, ExtOut> {
    fn process_transport_output<'a>(&mut self, now: Instant, out: TransportEvent<'a, ExtOut>) -> Option<Output<'a, ExtOut>> {
        if let TransportEvent::Ext(ext) = out {
            Some(Output::Ext(ext))
        } else {
            let out = self.internal.on_transport_event(now, out)?;
            self.process_internal_output(now, out)
        }
    }

    fn process_internal_output<'a>(&mut self, now: Instant, out: internal::InternalOutput<ExtOut>) -> Option<Output<'a, ExtOut>> {
        todo!()
    }
}
