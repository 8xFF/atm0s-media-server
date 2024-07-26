use std::{
    collections::VecDeque,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Instant,
};

use media_server_core::endpoint::{Endpoint, EndpointCfg, EndpointInput, EndpointOutput};
use media_server_protocol::transport::RpcResult;
use media_server_secure::MediaEdgeSecure;
use media_server_utils::Small2dMap;
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    group_owner_type, TaskGroup, TaskSwitcherChild,
};

use crate::transport::{RtpExtIn, RtpExtOut, TransportRtp, VariantParams};

group_owner_type!(RtpSession);

pub enum RtpGroupIn {
    Net(BackendIncoming),
    Ext(RtpExtIn),
    Close(),
}

#[derive(Debug)]
pub enum RtpGroupOut {
    Net(BackendOutgoing),
    Ext(RtpExtOut),
    Shutdown(),
    Continue,
}

pub struct MediaRtpWorker<ES: 'static + MediaEdgeSecure> {
    public_ip: IpAddr,
    available_slots: VecDeque<usize>,
    addr_slots: Small2dMap<SocketAddr, usize>,
    task_slots: Small2dMap<usize, usize>,
    queue: VecDeque<RtpGroupOut>,
    endpoints: TaskGroup<EndpointInput<RtpExtIn>, EndpointOutput<RtpExtOut>, Endpoint<TransportRtp<ES>, RtpExtIn, RtpExtOut>, 16>,
    secure: Arc<ES>,
}

impl<ES: 'static + MediaEdgeSecure> MediaRtpWorker<ES> {
    pub fn new(addrs: Vec<SocketAddr>, addrs_alt: Vec<SocketAddr>, secure: Arc<ES>) -> Self {
        Self {
            public_ip: addrs[0].ip(),
            available_slots: VecDeque::default(),
            addr_slots: Small2dMap::default(),
            task_slots: Small2dMap::default(),
            queue: VecDeque::from(addrs.iter().map(|addr| RtpGroupOut::Net(BackendOutgoing::UdpListen { addr: *addr, reuse: false })).collect::<Vec<_>>()),
            endpoints: TaskGroup::default(),
            secure,
        }
    }

    pub fn spawn(&mut self, session_id: u64, params: VariantParams, offer: &str) -> RpcResult<(usize, String)> {
        let slot = self.available_slots.pop_front().expect("not have available slot");
        let local_addr = self.addr_slots.get2(&slot).expect("undefine addr for slot");
        // let ip = local_addr.ip();
        let port = local_addr.port();

        let (trans, remote_addr, sdp) = TransportRtp::<ES>::new(params, offer, self.public_ip, port)?;
        let ep = Endpoint::new(
            session_id,
            EndpointCfg {
                max_egress_bitrate: 2_500_000,
                max_ingress_bitrate: 2_500_000,
                record: false,
            },
            trans,
        );
        let idx = self.endpoints.add_task(ep);
        self.task_slots.insert(slot, idx);
        Ok((idx, sdp))
    }

    fn process_output(&mut self, now: Instant, out: EndpointOutput<RtpExtOut>) -> RtpGroupOut {
        match out {
            _ => RtpGroupOut::Continue,
        }
    }
}

impl<ES: MediaEdgeSecure> MediaRtpWorker<ES> {
    pub fn tasks(&self) -> usize {
        self.endpoints.tasks()
    }

    pub fn on_tick(&mut self, now: Instant) {
        self.endpoints.on_tick(now);
    }

    pub fn on_event(&mut self, now: Instant, input: RtpGroupIn) {
        match input {
            RtpGroupIn::Net(BackendIncoming::UdpListenResult { bind: _, result }) => {
                let (addr, slot) = result.expect("Should listen ok");
                log::info!("[MediaRtpWorker] UdpListenResult {addr}, slot {slot}");
                self.available_slots.push_back(slot);
                self.addr_slots.insert(addr, slot);
            }
            RtpGroupIn::Net(BackendIncoming::UdpPacket { slot, from, data }) => match self.task_slots.get1(&slot) {
                Some(idx) => {
                    self.endpoints.on_event(now, *idx, EndpointInput::Net(BackendIncoming::UdpPacket { slot, from, data }));
                }
                None => {}
            },
            RtpGroupIn::Ext(ext) => match ext {
                RtpExtIn::Ping(id) => {
                    self.queue.push_back(RtpGroupOut::Ext(RtpExtOut::Pong(id, Result::Ok("pong".to_string()))));
                }
            },
            _ => {}
        }
    }

    pub fn shutdown(&mut self, now: Instant) {
        self.endpoints.on_shutdown(now);
    }
}

impl<ES: MediaEdgeSecure> TaskSwitcherChild<RtpGroupOut> for MediaRtpWorker<ES> {
    type Time = Instant;
    fn pop_output(&mut self, now: Self::Time) -> Option<RtpGroupOut> {
        self.queue.pop_front()
    }
}
