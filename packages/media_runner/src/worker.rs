use std::{collections::VecDeque, net::SocketAddr, time::Instant};

use atm0s_sdn::{services::visualization, NetInput, NetOutput, SdnExtIn, SdnExtOut, SdnWorker, SdnWorkerBusEvent, SdnWorkerCfg, SdnWorkerInput, SdnWorkerOutput, TimePivot};
use media_server_core::cluster::{self, MediaCluster};
use media_server_protocol::transport::{
    whep::{self, WhepConnectRes, WhepDeleteRes, WhepRemoteIceRes},
    whip::{self, WhipConnectRes, WhipDeleteRes, WhipRemoteIceRes},
    RpcReq, RpcRes,
};
use rand::random;
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    TaskSwitcher,
};
use transport_webrtc::{GroupInput, MediaWorkerWebrtc, WebrtcOwner};

pub struct MediaConfig {
    pub webrtc_addrs: Vec<SocketAddr>,
}

pub type SdnConfig = SdnWorkerCfg<SC, SE, TC, TW>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Sdn,
    MediaWebrtc,
}

//for sdn
pub type SC = visualization::Control;
pub type SE = visualization::Event;
pub type TC = ();
pub type TW = ();

pub enum Input<'a> {
    ExtRpc(u64, RpcReq<usize>),
    Net(Owner, BackendIncoming<'a>),
}

pub enum Output<'a> {
    ExtRpc(u64, RpcRes<usize>),
    ExtSdn(SdnExtOut<SE>),
    Net(Owner, BackendOutgoing<'a>),
    Bus(SdnWorkerBusEvent<SC, SE, TC, TW>),
    Continue,
}

#[repr(u8)]
enum TaskType {
    Sdn = 0,
    MediaCluster = 1,
    MediaWebrtc = 2,
}

impl TryFrom<usize> for TaskType {
    type Error = ();
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Sdn),
            1 => Ok(Self::MediaCluster),
            2 => Ok(Self::MediaWebrtc),
            _ => Err(()),
        }
    }
}

#[derive(convert_enum::From, Debug, Clone, Hash, PartialEq, Eq)]
enum MediaClusterOwner {
    Webrtc(WebrtcOwner),
}

pub struct MediaServerWorker {
    sdn_slot: usize,
    sdn_worker: SdnWorker<SC, SE, TC, TW>,
    media_cluster: MediaCluster<MediaClusterOwner>,
    media_webrtc: MediaWorkerWebrtc,
    switcher: TaskSwitcher,
    queue: VecDeque<Output<'static>>,
    timer: TimePivot,
}

impl MediaServerWorker {
    pub fn new(sdn: SdnConfig, media: MediaConfig) -> Self {
        Self {
            sdn_slot: 0,
            sdn_worker: SdnWorker::new(sdn),
            media_cluster: MediaCluster::default(),
            media_webrtc: MediaWorkerWebrtc::new(media.webrtc_addrs),
            switcher: TaskSwitcher::new(3),
            queue: VecDeque::new(),
            timer: TimePivot::build(),
        }
    }

    pub fn tasks(&self) -> usize {
        self.sdn_worker.tasks() + self.sdn_worker.tasks()
    }

    pub fn on_tick<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        let s = &mut self.switcher;
        while let Some(c) = s.looper_current(now) {
            match c.try_into().ok()? {
                TaskType::Sdn => {
                    let now_ms = self.timer.timestamp_ms(now);
                    if let Some(out) = s.looper_process(self.sdn_worker.on_tick(now_ms)) {
                        return Some(self.output_sdn(now, out));
                    }
                }
                TaskType::MediaCluster => {
                    if let Some(out) = s.looper_process(self.media_cluster.on_tick(now)) {
                        return Some(self.output_cluster(now, out));
                    }
                }
                TaskType::MediaWebrtc => {
                    if let Some(out) = s.looper_process(self.media_webrtc.on_tick(now)) {
                        return Some(self.output_webrtc(now, out));
                    }
                }
            }
        }
        None
    }

    pub fn on_event<'a>(&mut self, now: Instant, input: Input<'a>) -> Option<Output<'a>> {
        match input {
            Input::ExtRpc(req_id, req) => self.process_rpc(now, req_id, req),
            Input::Net(owner, event) => match owner {
                Owner::Sdn => {
                    let now_ms = self.timer.timestamp_ms(now);
                    match event {
                        BackendIncoming::UdpPacket { slot: _, from, data } => {
                            let out = self.sdn_worker.on_event(now_ms, SdnWorkerInput::Net(NetInput::UdpPacket(from, data)))?;
                            Some(self.output_sdn(now, out))
                        }
                        BackendIncoming::UdpListenResult { bind: _, result } => {
                            let (_addr, slot) = result.ok()?;
                            self.sdn_slot = slot;
                            None
                        }
                    }
                }
                Owner::MediaWebrtc => {
                    let out = self.media_webrtc.on_event(now, transport_webrtc::GroupInput::Net(event))?;
                    Some(self.output_webrtc(now, out))
                }
            },
        }
    }

    pub fn pop_output<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        if let Some(out) = self.queue.pop_front() {
            return Some(out);
        }
        let s = &mut self.switcher;
        while let Some(c) = s.queue_current() {
            match c.try_into().ok()? {
                TaskType::Sdn => {
                    let now_ms = self.timer.timestamp_ms(now);
                    if let Some(out) = s.queue_process(self.sdn_worker.pop_output(now_ms)) {
                        return Some(self.output_sdn(now, out));
                    }
                }
                TaskType::MediaCluster => {
                    if let Some(out) = s.queue_process(self.media_cluster.pop_output(now)) {
                        return Some(self.output_cluster(now, out));
                    }
                }
                TaskType::MediaWebrtc => {
                    if let Some(out) = s.queue_process(self.media_webrtc.pop_output(now)) {
                        return Some(self.output_webrtc(now, out));
                    }
                }
            }
        }
        None
    }

    pub fn shutdown<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        let s = &mut self.switcher;
        while let Some(c) = s.looper_current(now) {
            match c.try_into().ok()? {
                TaskType::Sdn => {
                    let now_ms = self.timer.timestamp_ms(now);
                    if let Some(out) = s.looper_process(self.sdn_worker.on_event(now_ms, SdnWorkerInput::ShutdownRequest)) {
                        return Some(self.output_sdn(now, out));
                    }
                }
                TaskType::MediaCluster => {
                    if let Some(out) = s.looper_process(self.media_cluster.shutdown(now)) {
                        return Some(self.output_cluster(now, out));
                    }
                }
                TaskType::MediaWebrtc => {
                    if let Some(out) = s.looper_process(self.media_webrtc.shutdown(now)) {
                        return Some(self.output_webrtc(now, out));
                    }
                }
            }
        }
        None
    }
}

impl MediaServerWorker {
    fn output_sdn<'a>(&mut self, now: Instant, out: SdnWorkerOutput<'a, SC, SE, TC, TW>) -> Output<'a> {
        self.switcher.queue_flag_task(TaskType::Sdn as usize);
        match out {
            SdnWorkerOutput::Ext(out) => Output::ExtSdn(out),
            SdnWorkerOutput::ExtWorker(out) => match out {
                SdnExtOut::FeaturesEvent(e) => {
                    if let Some(out) = self.media_cluster.on_sdn_event(now, e) {
                        self.output_cluster(now, out)
                    } else {
                        Output::Continue
                    }
                }
                SdnExtOut::ServicesEvent(..) => Output::Continue,
            },
            SdnWorkerOutput::Net(out) => match out {
                NetOutput::UdpPacket(to, data) => Output::Net(Owner::Sdn, BackendOutgoing::UdpPacket { slot: self.sdn_slot, to, data }),
                NetOutput::UdpPackets(to, data) => Output::Net(Owner::Sdn, BackendOutgoing::UdpPackets { slot: self.sdn_slot, to, data }),
            },
            SdnWorkerOutput::Bus(event) => Output::Bus(event),
            SdnWorkerOutput::ShutdownResponse => Output::Continue,
            SdnWorkerOutput::Continue => Output::Continue,
        }
    }

    fn output_cluster<'a>(&mut self, now: Instant, out: cluster::Output<MediaClusterOwner>) -> Output<'a> {
        self.switcher.queue_flag_task(TaskType::MediaCluster as usize);
        match out {
            cluster::Output::Sdn(control) => {
                let now_ms = self.timer.timestamp_ms(now);
                if let Some(out) = self.sdn_worker.on_event(now_ms, SdnWorkerInput::ExtWorker(SdnExtIn::FeaturesControl(control))) {
                    self.output_sdn(now, out)
                } else {
                    Output::Continue
                }
            }
            cluster::Output::Endpoint(owners, event) => {
                for owner in owners {
                    match owner {
                        MediaClusterOwner::Webrtc(owner) => {
                            if let Some(out) = self.media_webrtc.on_event(now, transport_webrtc::GroupInput::Cluster(owner, event.clone())) {
                                let out = self.output_webrtc(now, out);
                                if !matches!(out, Output::Continue) {
                                    self.queue.push_back(out);
                                }
                            }
                        }
                    }
                }
                Output::Continue
            }
        }
    }

    fn output_webrtc<'a>(&mut self, now: Instant, out: transport_webrtc::GroupOutput<'a, transport_webrtc::ExtOut>) -> Output<'a> {
        self.switcher.queue_flag_task(TaskType::MediaWebrtc as usize);
        match out {
            transport_webrtc::GroupOutput::Net(out) => Output::Net(Owner::MediaWebrtc, out),
            transport_webrtc::GroupOutput::Cluster(owner, control) => {
                if let Some(out) = self.media_cluster.on_endpoint_control(now, owner.into(), control) {
                    self.output_cluster(now, out)
                } else {
                    Output::Continue
                }
            }
            transport_webrtc::GroupOutput::Shutdown(_owner) => Output::Continue,
            transport_webrtc::GroupOutput::Ext(_owner, ext) => match ext {
                transport_webrtc::ExtOut::RemoteIce(req_id, variant, res) => match variant {
                    transport_webrtc::Variant::Whip => Output::ExtRpc(req_id, RpcRes::Whip(whip::RpcRes::RemoteIce(res.map(|_| WhipRemoteIceRes {})))),
                    transport_webrtc::Variant::Whep => Output::ExtRpc(req_id, RpcRes::Whep(whep::RpcRes::RemoteIce(res.map(|_| WhepRemoteIceRes {})))),
                    transport_webrtc::Variant::Sdk => {
                        todo!()
                    }
                },
            },
        }
    }
}

impl MediaServerWorker {
    fn process_rpc<'a>(&mut self, now: Instant, req_id: u64, req: RpcReq<usize>) -> Option<Output<'a>> {
        log::info!("[MediaServerWorker] incoming rpc req {req_id}");
        match req {
            RpcReq::Whip(req) => match req {
                whip::RpcReq::Connect(req) => match self
                    .media_webrtc
                    .spawn(transport_webrtc::VariantParams::Whip(req.token.into(), "publisher".to_string().into()), &req.sdp)
                {
                    Ok((sdp, conn_id)) => Some(Output::ExtRpc(req_id, RpcRes::Whip(whip::RpcRes::Connect(Ok(WhipConnectRes { conn_id, sdp }))))),
                    Err(e) => Some(Output::ExtRpc(req_id, RpcRes::Whip(whip::RpcRes::Connect(Err(e))))),
                },
                whip::RpcReq::RemoteIce(req) => {
                    log::info!("on rpc request {req_id}, whip::RpcReq::RemoteIce");
                    let out = self.media_webrtc.on_event(
                        now,
                        GroupInput::Ext(req.conn_id.into(), transport_webrtc::ExtIn::RemoteIce(req_id, transport_webrtc::Variant::Whip, req.ice)),
                    )?;
                    Some(self.output_webrtc(now, out))
                }
                whip::RpcReq::Delete(req) => {
                    //TODO check error instead of auto response ok
                    self.queue.push_back(Output::ExtRpc(req_id, RpcRes::Whip(whip::RpcRes::Delete(Ok(WhipDeleteRes {})))));
                    let out = self.media_webrtc.on_event(now, GroupInput::Close(req.conn_id.into()))?;
                    Some(self.output_webrtc(now, out))
                }
            },
            RpcReq::Whep(req) => match req {
                whep::RpcReq::Connect(req) => {
                    let peer_id = format!("whep-{}", random::<u64>());
                    match self.media_webrtc.spawn(transport_webrtc::VariantParams::Whep(req.token.into(), peer_id.into()), &req.sdp) {
                        Ok((sdp, conn_id)) => Some(Output::ExtRpc(req_id, RpcRes::Whep(whep::RpcRes::Connect(Ok(WhepConnectRes { conn_id, sdp }))))),
                        Err(e) => Some(Output::ExtRpc(req_id, RpcRes::Whep(whep::RpcRes::Connect(Err(e))))),
                    }
                }
                whep::RpcReq::RemoteIce(req) => {
                    log::info!("on rpc request {req_id}, whep::RpcReq::RemoteIce");
                    let out = self.media_webrtc.on_event(
                        now,
                        GroupInput::Ext(req.conn_id.into(), transport_webrtc::ExtIn::RemoteIce(req_id, transport_webrtc::Variant::Whep, req.ice)),
                    )?;
                    Some(self.output_webrtc(now, out))
                }
                whep::RpcReq::Delete(req) => {
                    //TODO check error instead of auto response ok
                    self.queue.push_back(Output::ExtRpc(req_id, RpcRes::Whep(whep::RpcRes::Delete(Ok(WhepDeleteRes {})))));
                    let out = self.media_webrtc.on_event(now, GroupInput::Close(req.conn_id.into()))?;
                    Some(self.output_webrtc(now, out))
                }
            },
        }
    }
}
