use std::{collections::VecDeque, net::SocketAddr, time::Instant};

use media_server_core::{
    cluster::{ClusterEndpointControl, ClusterEndpointEvent, ClusterRoomHash},
    endpoint::{Endpoint, EndpointCfg, EndpointInput, EndpointOutput},
};
use media_server_protocol::transport::{RpcError, RpcResult};
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    group_owner_type, group_task, return_if_none, return_if_some, TaskSwitcher, TaskSwitcherChild,
};
use str0m::change::DtlsCert;

use crate::{
    shared_port::SharedUdpPort,
    transport::{ExtIn, ExtOut, TransportWebrtc, VariantParams},
    WebrtcError,
};

group_task!(Endpoints, Endpoint<TransportWebrtc, ExtIn, ExtOut>, EndpointInput<ExtIn>, EndpointOutput<ExtOut>);
group_owner_type!(WebrtcOwner);

pub enum GroupInput {
    Net(BackendIncoming),
    Cluster(WebrtcOwner, ClusterEndpointEvent),
    Ext(WebrtcOwner, ExtIn),
    Close(WebrtcOwner),
}

#[derive(Debug)]
pub enum GroupOutput {
    Net(BackendOutgoing),
    Cluster(WebrtcOwner, ClusterRoomHash, ClusterEndpointControl),
    Ext(WebrtcOwner, ExtOut),
    Shutdown(WebrtcOwner),
    Continue,
}

pub struct MediaWorkerWebrtc {
    ice_lite: bool,
    shared_port: SharedUdpPort<usize>,
    dtls_cert: DtlsCert,
    endpoints: Endpoints,
    addrs: Vec<(SocketAddr, usize)>,
    queue: VecDeque<GroupOutput>,
}

impl MediaWorkerWebrtc {
    pub fn new(addrs: Vec<SocketAddr>, ice_lite: bool) -> Self {
        Self {
            ice_lite,
            shared_port: SharedUdpPort::default(),
            dtls_cert: DtlsCert::new_openssl(),
            endpoints: Endpoints::default(),
            addrs: vec![],
            queue: VecDeque::from(addrs.iter().map(|addr| GroupOutput::Net(BackendOutgoing::UdpListen { addr: *addr, reuse: false })).collect::<Vec<_>>()),
        }
    }

    pub fn spawn(&mut self, variant: VariantParams, offer: &str) -> RpcResult<(bool, String, usize)> {
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
        let (tran, ufrag, sdp) = TransportWebrtc::new(variant, offer, self.dtls_cert.clone(), self.addrs.clone(), self.ice_lite)?;
        let endpoint = Endpoint::new(cfg, tran);
        let index = self.endpoints.add_task(endpoint);
        self.shared_port.add_ufrag(ufrag, index);
        Ok((self.ice_lite, sdp, index))
    }

    fn process_output(&mut self, index: usize, out: EndpointOutput<ExtOut>) -> GroupOutput {
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

    pub fn on_tick(&mut self, now: Instant) {
        self.endpoints.on_tick(now);
    }

    pub fn on_event(&mut self, now: Instant, input: GroupInput) {
        match input {
            GroupInput::Net(BackendIncoming::UdpListenResult { bind: _, result }) => {
                let (addr, slot) = result.expect("Should listen ok");
                log::info!("[MediaWorkerWebrtc] UdpListenResult {addr}, slot {slot}");
                self.addrs.push((addr, slot));
            }
            GroupInput::Net(BackendIncoming::UdpPacket { slot, from, data }) => {
                let index = return_if_none!(self.shared_port.map_remote(from, &data));
                self.endpoints.on_event(now, index, EndpointInput::Net(BackendIncoming::UdpPacket { slot, from, data }));
            }
            GroupInput::Cluster(owner, event) => {
                self.endpoints.on_event(now, owner.index(), EndpointInput::Cluster(event));
            }
            GroupInput::Ext(owner, ext) => {
                log::info!("[MediaWorkerWebrtc] on ext to owner {:?}", owner);
                if let Some(&Some(_)) = self.endpoints.tasks.get(owner.index()) {
                    self.endpoints.on_event(now, owner.index(), EndpointInput::Ext(ext));
                } else {
                    match ext {
                        ExtIn::RemoteIce(req_id, variant, ..) => {
                            self.queue
                                .push_back(GroupOutput::Ext(owner, ExtOut::RemoteIce(req_id, variant, Err(RpcError::new2(WebrtcError::RpcEndpointNotFound)))));
                        }
                        ExtIn::RestartIce(req_id, variant, remote, useragent, token, req) => {
                            if let Ok((ice_lite, sdp, index)) = self.spawn(VariantParams::Webrtc(remote, useragent, token, req.clone()), &req.sdp) {
                                self.queue.push_back(GroupOutput::Ext(index.into(), ExtOut::RestartIce(req_id, variant, Ok((ice_lite, sdp)))));
                            } else {
                                self.queue
                                    .push_back(GroupOutput::Ext(owner, ExtOut::RestartIce(req_id, variant, Err(RpcError::new2(WebrtcError::RpcEndpointNotFound)))));
                            }
                        }
                    }
                }
            }
            GroupInput::Close(owner) => {
                self.endpoints.on_event(now, owner.index(), EndpointInput::Close);
            }
        }
    }

    pub fn shutdown(&mut self, now: Instant) {
        self.endpoints.on_shutdown(now);
    }
}

impl TaskSwitcherChild<GroupOutput> for MediaWorkerWebrtc {
    type Time = Instant;
    fn pop_output(&mut self, now: Instant) -> Option<GroupOutput> {
        return_if_some!(self.queue.pop_front());
        let (index, out) = self.endpoints.pop_output(now)?;
        Some(self.process_output(index, out))
    }
}
