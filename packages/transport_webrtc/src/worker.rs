use std::{collections::VecDeque, net::SocketAddr, sync::Arc, time::Instant};

use media_server_core::{
    cluster::{ClusterEndpointControl, ClusterEndpointEvent, ClusterRoomHash},
    endpoint::{Endpoint, EndpointCfg, EndpointInput, EndpointOutput},
};
use media_server_protocol::transport::{RpcError, RpcResult};
use media_server_secure::MediaEdgeSecure;
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    group_owner_type, return_if_none, return_if_some, TaskGroup, TaskSwitcherChild,
};
use str0m::change::DtlsCert;

use crate::{
    shared_port::SharedUdpPort,
    transport::{ExtIn, ExtOut, TransportWebrtc, VariantParams},
    WebrtcError,
};

group_owner_type!(WebrtcSession);

pub enum GroupInput {
    Net(BackendIncoming),
    Cluster(WebrtcSession, ClusterEndpointEvent),
    Ext(WebrtcSession, ExtIn),
    Close(WebrtcSession),
}

#[derive(Debug)]
pub enum GroupOutput {
    Net(BackendOutgoing),
    Cluster(WebrtcSession, ClusterRoomHash, ClusterEndpointControl),
    Ext(WebrtcSession, ExtOut),
    Shutdown(WebrtcSession),
    Continue,
}

#[allow(clippy::type_complexity)]
pub struct MediaWorkerWebrtc<ES: 'static + MediaEdgeSecure> {
    ice_lite: bool,
    shared_port: SharedUdpPort<usize>,
    dtls_cert: DtlsCert,
    endpoints: TaskGroup<EndpointInput<ExtIn>, EndpointOutput<ExtOut>, Endpoint<TransportWebrtc<ES>, ExtIn, ExtOut>, 16>,
    addrs: Vec<(SocketAddr, usize)>,
    queue: VecDeque<GroupOutput>,
    secure: Arc<ES>,
}

impl<ES: MediaEdgeSecure> MediaWorkerWebrtc<ES> {
    pub fn new(addrs: Vec<SocketAddr>, ice_lite: bool, secure: Arc<ES>) -> Self {
        Self {
            ice_lite,
            shared_port: SharedUdpPort::default(),
            dtls_cert: DtlsCert::new_openssl(),
            endpoints: TaskGroup::default(),
            addrs: vec![],
            queue: VecDeque::from(addrs.iter().map(|addr| GroupOutput::Net(BackendOutgoing::UdpListen { addr: *addr, reuse: false })).collect::<Vec<_>>()),
            secure,
        }
    }

    pub fn spawn(&mut self, variant: VariantParams<ES>, offer: &str) -> RpcResult<(bool, String, usize)> {
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
            EndpointOutput::Cluster(room, control) => GroupOutput::Cluster(WebrtcSession(index), room, control),
            EndpointOutput::Destroy => {
                self.endpoints.remove_task(index);
                self.shared_port.remove_task(index);
                GroupOutput::Shutdown(WebrtcSession(index))
            }
            EndpointOutput::Ext(ext) => GroupOutput::Ext(WebrtcSession(index), ext),
            EndpointOutput::Continue => GroupOutput::Continue,
        }
    }
}

impl<ES: MediaEdgeSecure> MediaWorkerWebrtc<ES> {
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
                if self.endpoints.has_task(owner.index()) {
                    self.endpoints.on_event(now, owner.index(), EndpointInput::Ext(ext));
                } else {
                    match ext {
                        ExtIn::RemoteIce(req_id, variant, ..) => {
                            self.queue
                                .push_back(GroupOutput::Ext(owner, ExtOut::RemoteIce(req_id, variant, Err(RpcError::new2(WebrtcError::RpcEndpointNotFound)))));
                        }
                        ExtIn::RestartIce(req_id, variant, remote, useragent, req) => {
                            let sdp = req.sdp.clone();
                            if let Ok((ice_lite, sdp, index)) = self.spawn(VariantParams::Webrtc(remote, useragent, req, self.secure.clone()), &sdp) {
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

impl<ES: MediaEdgeSecure> TaskSwitcherChild<GroupOutput> for MediaWorkerWebrtc<ES> {
    type Time = Instant;
    fn pop_output(&mut self, now: Instant) -> Option<GroupOutput> {
        return_if_some!(self.queue.pop_front());
        let (index, out) = self.endpoints.pop_output(now)?;
        Some(self.process_output(index, out))
    }
}
