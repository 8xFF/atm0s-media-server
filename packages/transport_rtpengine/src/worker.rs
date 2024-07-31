use std::{collections::VecDeque, net::IpAddr, time::Instant};

use media_server_core::{
    cluster::{ClusterEndpointControl, ClusterEndpointEvent, ClusterRoomHash},
    endpoint::{Endpoint, EndpointCfg, EndpointInput, EndpointOutput},
};
use media_server_protocol::{
    endpoint::{PeerId, RoomId},
    protobuf::cluster_connector::peer_event,
    record::SessionRecordEvent,
    transport::{RpcError, RpcResult},
};
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    group_owner_type, return_if_some, TaskGroup, TaskSwitcherChild,
};

use crate::transport::{ExtIn, ExtOut, TransportRtpEngine};

group_owner_type!(RtpEngineSession);

#[allow(clippy::large_enum_variant)]
pub enum GroupInput {
    Net(usize, BackendIncoming),
    Cluster(RtpEngineSession, ClusterEndpointEvent),
    Ext(RtpEngineSession, ExtIn),
}

#[derive(Debug)]
pub enum GroupOutput {
    Net(usize, BackendOutgoing),
    Cluster(RtpEngineSession, ClusterRoomHash, ClusterEndpointControl),
    PeerEvent(RtpEngineSession, u64, Instant, peer_event::Event),
    RecordEvent(RtpEngineSession, u64, Instant, SessionRecordEvent),
    Ext(RtpEngineSession, ExtOut),
    Shutdown(RtpEngineSession),
    Continue,
}

#[allow(clippy::type_complexity)]
pub struct MediaWorkerRtpEngine {
    ip: IpAddr,
    endpoints: TaskGroup<EndpointInput<ExtIn>, EndpointOutput<ExtOut>, Endpoint<TransportRtpEngine, ExtIn, ExtOut>, 16>,
    queue: VecDeque<GroupOutput>,
}

impl MediaWorkerRtpEngine {
    pub fn new(ip: IpAddr) -> Self {
        Self {
            ip,
            endpoints: TaskGroup::default(),
            queue: VecDeque::new(),
        }
    }

    pub fn spawn(&mut self, room: RoomId, peer: PeerId, record: bool, session_id: u64, offer: &str) -> RpcResult<(usize, String)> {
        let (tran, answer) = TransportRtpEngine::new(room, peer, self.ip, offer).map_err(|e| RpcError::new(1000_u32, &e))?;
        let cfg = EndpointCfg {
            max_ingress_bitrate: 2_500_000,
            max_egress_bitrate: 2_500_000,
            record,
        };
        let endpoint = Endpoint::new(session_id, cfg, tran);
        let index = self.endpoints.add_task(endpoint);
        Ok((index, answer))
    }

    fn process_output(&mut self, index: usize, out: EndpointOutput<ExtOut>) -> GroupOutput {
        match out {
            EndpointOutput::Net(net) => GroupOutput::Net(index, net),
            EndpointOutput::Cluster(room, control) => GroupOutput::Cluster(RtpEngineSession(index), room, control),
            EndpointOutput::PeerEvent(session_id, ts, event) => GroupOutput::PeerEvent(RtpEngineSession(index), session_id, ts, event),
            EndpointOutput::RecordEvent(session_id, ts, event) => GroupOutput::RecordEvent(RtpEngineSession(index), session_id, ts, event),
            EndpointOutput::Destroy => {
                log::info!("[TransportRtpEngine] destroy endpoint {index}");
                self.endpoints.remove_task(index);
                GroupOutput::Shutdown(RtpEngineSession(index))
            }
            EndpointOutput::Ext(ext) => GroupOutput::Ext(RtpEngineSession(index), ext),
            EndpointOutput::Continue => GroupOutput::Continue,
        }
    }
}

impl MediaWorkerRtpEngine {
    pub fn tasks(&self) -> usize {
        self.endpoints.tasks()
    }

    pub fn on_tick(&mut self, now: Instant) {
        self.endpoints.on_tick(now);
    }

    pub fn on_event(&mut self, now: Instant, input: GroupInput) {
        match input {
            GroupInput::Net(child, event) => {
                self.endpoints.on_event(now, child, EndpointInput::Net(event));
            }
            GroupInput::Cluster(owner, event) => {
                self.endpoints.on_event(now, owner.index(), EndpointInput::Cluster(event));
            }
            GroupInput::Ext(owner, ext) => {
                log::info!("[MediaWorkerRtpEngine] on ext to owner {:?}", owner);
                match ext {
                    ExtIn::Disconnect(req_id) => {
                        self.endpoints.on_event(now, owner.index(), EndpointInput::Ext(ExtIn::Disconnect(req_id)));
                    }
                }
            }
        }
    }

    pub fn shutdown(&mut self, now: Instant) {
        self.endpoints.on_shutdown(now);
    }
}

impl TaskSwitcherChild<GroupOutput> for MediaWorkerRtpEngine {
    type Time = Instant;
    fn pop_output(&mut self, now: Instant) -> Option<GroupOutput> {
        return_if_some!(self.queue.pop_front());
        let (index, out) = self.endpoints.pop_output(now)?;
        Some(self.process_output(index, out))
    }
}
