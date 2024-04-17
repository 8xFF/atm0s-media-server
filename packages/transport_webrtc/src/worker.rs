use std::{collections::VecDeque, net::SocketAddr, time::Instant};

use media_server_core::{
    cluster::{EndpointControl, EndpointEvent},
    endpoint::{Endpoint, Input, Output},
};
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    group_owner_type, group_task, TaskSwitcher,
};
use str0m::change::DtlsCert;

use crate::{
    shared_port::SharedUdpPort,
    transport::{TransportWebrtc, Variant},
};

group_task!(Endpoints, Endpoint<TransportWebrtc>, Input<'a>, Output<'a>);

group_owner_type!(WebrtcOwner);

pub enum GroupInput<'a> {
    Net(BackendIncoming<'a>),
    Cluster(WebrtcOwner, EndpointEvent),
}

pub enum GroupOutput<'a> {
    Net(BackendOutgoing<'a>),
    Cluster(WebrtcOwner, EndpointControl),
    Shutdown(WebrtcOwner),
}

pub struct MediaWorkerWebrtc {
    shared_port: SharedUdpPort<usize>,
    dtls_cert: DtlsCert,
    endpoints: Endpoints,
    addrs: Vec<(SocketAddr, usize)>,
    queue: VecDeque<GroupOutput<'static>>,
}

impl MediaWorkerWebrtc {
    pub fn new(addrs: Vec<SocketAddr>) -> Self {
        Self {
            shared_port: SharedUdpPort::default(),
            dtls_cert: DtlsCert::new_openssl(),
            endpoints: Endpoints::default(),
            addrs: vec![],
            queue: VecDeque::from(addrs.iter().map(|addr| GroupOutput::Net(BackendOutgoing::UdpListen { addr: *addr, reuse: false })).collect::<Vec<_>>()),
        }
    }

    pub fn spawn(&mut self, variant: Variant, offer: &str) -> Option<String> {
        let (tran, ufrag, sdp) = TransportWebrtc::new(variant, offer, self.dtls_cert.clone(), self.addrs.clone()).ok()?;
        let endpoint = Endpoint::new(tran);
        let index = self.endpoints.add_task(endpoint);
        self.shared_port.add_ufrag(ufrag, index);
        Some(sdp)
    }

    fn process_output<'a>(&mut self, index: usize, out: Output<'a>) -> GroupOutput<'a> {
        match out {
            Output::Net(net) => GroupOutput::Net(net),
            Output::Cluster(control) => GroupOutput::Cluster(WebrtcOwner(index), control),
            Output::Shutdown => {
                self.endpoints.remove_task(index);
                self.shared_port.remove_task(index);
                GroupOutput::Shutdown(WebrtcOwner(index))
            }
        }
    }
}

impl MediaWorkerWebrtc {
    pub fn tasks(&self) -> usize {
        self.endpoints.tasks()
    }

    pub fn on_tick<'a>(&mut self, now: Instant) -> Option<GroupOutput<'a>> {
        if let Some(out) = self.queue.pop_front() {
            return Some(out);
        }
        self.endpoints.on_tick(now).map(|(index, out)| self.process_output(index, out))
    }

    pub fn on_event<'a>(&mut self, now: Instant, input: GroupInput<'a>) -> Option<GroupOutput<'a>> {
        match input {
            GroupInput::Net(BackendIncoming::UdpListenResult { bind: _, result }) => {
                let (addr, slot) = result.ok()?;
                self.addrs.push((addr, slot));
                None
            }
            GroupInput::Net(BackendIncoming::UdpPacket { slot, from, data }) => {
                let index = self.shared_port.map_remote(from, &data)?;
                let out = self.endpoints.on_event(now, index, Input::Net(BackendIncoming::UdpPacket { slot, from, data }))?;
                Some(self.process_output(index, out))
            }
            GroupInput::Cluster(owner, event) => {
                let out = self.endpoints.on_event(now, owner.index(), Input::Cluster(event))?;
                Some(self.process_output(owner.index(), out))
            }
        }
    }

    pub fn pop_output<'a>(&mut self, now: Instant) -> Option<GroupOutput<'a>> {
        let (index, out) = self.endpoints.pop_output(now)?;
        Some(self.process_output(index, out))
    }

    pub fn shutdown<'a>(&mut self, now: Instant) -> Option<GroupOutput<'a>> {
        self.endpoints.shutdown(now).map(|(index, out)| self.process_output(index, out))
    }
}
