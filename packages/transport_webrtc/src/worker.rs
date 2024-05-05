use std::{collections::VecDeque, net::SocketAddr, time::Instant};

use media_server_core::{
    cluster::{ClusterEndpointControl, ClusterEndpointEvent, ClusterRoomHash},
    endpoint::{Endpoint, EndpointCfg, EndpointInput, EndpointOutput},
};
use media_server_protocol::transport::RpcResult;
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    group_owner_type, group_task, TaskSwitcher,
};
use str0m::change::DtlsCert;

use crate::{
    shared_port::SharedUdpPort,
    transport::{ExtIn, ExtOut, TransportWebrtc, VariantParams},
};

group_task!(Endpoints, Endpoint<TransportWebrtc, ExtIn, ExtOut>, EndpointInput<'a, ExtIn>, EndpointOutput<'a, ExtOut>);

group_owner_type!(WebrtcOwner);

pub enum GroupInput<'a, Ext> {
    Net(BackendIncoming<'a>),
    Cluster(WebrtcOwner, ClusterEndpointEvent),
    Ext(WebrtcOwner, Ext),
    Close(WebrtcOwner),
}

pub enum GroupOutput<'a, Ext> {
    Net(BackendOutgoing<'a>),
    Cluster(WebrtcOwner, ClusterRoomHash, ClusterEndpointControl),
    Ext(WebrtcOwner, Ext),
    Shutdown(WebrtcOwner),
    Continue,
}

pub struct MediaWorkerWebrtc {
    shared_port: SharedUdpPort<usize>,
    dtls_cert: DtlsCert,
    endpoints: Endpoints,
    addrs: Vec<(SocketAddr, usize)>,
    queue: VecDeque<GroupOutput<'static, ExtOut>>,
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

    pub fn spawn(&mut self, variant: VariantParams, offer: &str) -> RpcResult<(String, usize)> {
        let cfg = match &variant {
            VariantParams::Whip(_, _) => EndpointCfg {
                max_ingress_bitrate: 2_500_000,
                max_egress_bitrate: 2_500_000,
            },
            VariantParams::Whep(_, _) => EndpointCfg {
                max_ingress_bitrate: 2_500_000,
                max_egress_bitrate: 2_500_000,
            },
            VariantParams::Webrtc(_, _, _, _) => EndpointCfg {
                max_ingress_bitrate: 2_500_000,
                max_egress_bitrate: 2_500_000,
            },
        };
        let (tran, ufrag, sdp) = TransportWebrtc::new(variant, offer, self.dtls_cert.clone(), self.addrs.clone())?;
        let endpoint = Endpoint::new(cfg, tran);
        let index = self.endpoints.add_task(endpoint);
        self.shared_port.add_ufrag(ufrag, index);
        Ok((sdp, index))
    }

    fn process_output<'a>(&mut self, index: usize, out: EndpointOutput<'a, ExtOut>) -> GroupOutput<'a, ExtOut> {
        match out {
            EndpointOutput::Net(net) => GroupOutput::Net(net),
            EndpointOutput::Cluster(room, control) => GroupOutput::Cluster(WebrtcOwner(index), room, control),
            EndpointOutput::Destroy => {
                self.endpoints.remove_task(index);
                self.shared_port.remove_task(index);
                GroupOutput::Shutdown(WebrtcOwner(index))
            }
            EndpointOutput::Ext(ext) => GroupOutput::Ext(WebrtcOwner(index), ext),
            EndpointOutput::Continue => GroupOutput::Continue,
        }
    }
}

impl MediaWorkerWebrtc {
    pub fn tasks(&self) -> usize {
        self.endpoints.tasks()
    }

    pub fn on_tick<'a>(&mut self, now: Instant) -> Option<GroupOutput<'a, ExtOut>> {
        if let Some(out) = self.queue.pop_front() {
            return Some(out);
        }
        self.endpoints.on_tick(now).map(|(index, out)| self.process_output(index, out))
    }

    pub fn on_event<'a>(&mut self, now: Instant, input: GroupInput<'a, ExtIn>) -> Option<GroupOutput<'a, ExtOut>> {
        match input {
            GroupInput::Net(BackendIncoming::UdpListenResult { bind: _, result }) => {
                let (addr, slot) = result.ok()?;
                log::info!("[MediaWorkerWebrtc] UdpListenResult {addr}, slot {slot}");
                self.addrs.push((addr, slot));
                None
            }
            GroupInput::Net(BackendIncoming::UdpPacket { slot, from, data }) => {
                let index = self.shared_port.map_remote(from, &data)?;
                let out = self.endpoints.on_event(now, index, EndpointInput::Net(BackendIncoming::UdpPacket { slot, from, data }))?;
                Some(self.process_output(index, out))
            }
            GroupInput::Cluster(owner, event) => {
                let out = self.endpoints.on_event(now, owner.index(), EndpointInput::Cluster(event))?;
                Some(self.process_output(owner.index(), out))
            }
            GroupInput::Ext(owner, ext) => {
                log::info!("[MediaWorkerWebrtc] on ext to owner {:?}", owner);
                let out = self.endpoints.on_event(now, owner.index(), EndpointInput::Ext(ext))?;
                Some(self.process_output(owner.index(), out))
            }
            GroupInput::Close(owner) => {
                let out = self.endpoints.on_event(now, owner.index(), EndpointInput::Close)?;
                Some(self.process_output(owner.index(), out))
            }
        }
    }

    pub fn pop_output<'a>(&mut self, now: Instant) -> Option<GroupOutput<'a, ExtOut>> {
        let (index, out) = self.endpoints.pop_output(now)?;
        Some(self.process_output(index, out))
    }

    pub fn shutdown<'a>(&mut self, now: Instant) -> Option<GroupOutput<'a, ExtOut>> {
        self.endpoints.shutdown(now).map(|(index, out)| self.process_output(index, out))
    }
}
