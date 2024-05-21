use std::{net::SocketAddr, time::Instant};

use atm0s_sdn::{services::visualization, NetInput, NetOutput, SdnExtIn, SdnExtOut, SdnWorker, SdnWorkerBusEvent, SdnWorkerCfg, SdnWorkerInput, SdnWorkerOutput, TimePivot};
use media_server_core::cluster::{self, MediaCluster};
use media_server_protocol::{
    protobuf::gateway::{ConnectResponse, RemoteIceResponse},
    transport::{
        webrtc,
        whep::{self, WhepConnectRes, WhepDeleteRes, WhepRemoteIceRes},
        whip::{self, WhipConnectRes, WhipDeleteRes, WhipRemoteIceRes},
        RpcReq, RpcRes,
    },
};
use rand::random;
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    collections::DynamicDeque,
    TaskSwitcher, TaskSwitcherBranch,
};
use transport_webrtc::{GroupInput, MediaWorkerWebrtc, VariantParams, WebrtcOwner};

pub struct MediaConfig {
    pub ice_lite: bool,
    pub webrtc_addrs: Vec<SocketAddr>,
}

pub type SdnConfig = SdnWorkerCfg<UserData, SC, SE, TC, TW>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Sdn,
    MediaWebrtc,
}

//for sdn
pub type UserData = cluster::ClusterRoomHash;
pub type SC = visualization::Control;
pub type SE = visualization::Event;
pub type TC = ();
pub type TW = ();

pub enum Input {
    ExtRpc(u64, RpcReq<usize>),
    ExtSdn(SdnExtIn<UserData, SC>),
    Net(Owner, BackendIncoming),
    Bus(SdnWorkerBusEvent<UserData, SC, SE, TC, TW>),
}

pub enum Output {
    ExtRpc(u64, RpcRes<usize>),
    ExtSdn(SdnExtOut<UserData, SE>),
    Net(Owner, BackendOutgoing),
    Bus(SdnWorkerBusEvent<UserData, SC, SE, TC, TW>),
    Continue,
}

#[derive(num_enum::TryFromPrimitive, num_enum::IntoPrimitive)]
#[repr(usize)]
enum TaskType {
    Sdn,
    MediaCluster,
    MediaWebrtc,
}

#[derive(convert_enum::From, Debug, Clone, Copy, Hash, PartialEq, Eq)]
enum MediaClusterOwner {
    Webrtc(WebrtcOwner),
}

pub struct MediaServerWorker {
    sdn_slot: usize,
    sdn_worker: TaskSwitcherBranch<SdnWorker<UserData, SC, SE, TC, TW>, SdnWorkerOutput<UserData, SC, SE, TC, TW>>,
    media_cluster: TaskSwitcherBranch<MediaCluster<MediaClusterOwner>, cluster::Output<MediaClusterOwner>>,
    media_webrtc: TaskSwitcherBranch<MediaWorkerWebrtc, transport_webrtc::GroupOutput>,
    switcher: TaskSwitcher,
    queue: DynamicDeque<Output, 16>,
    timer: TimePivot,
}

impl MediaServerWorker {
    pub fn new(udp_port: u16, sdn: SdnConfig, media: MediaConfig) -> Self {
        let sdn_udp_addr = SocketAddr::from(([0, 0, 0, 0], udp_port));
        Self {
            sdn_slot: 1, //TODO dont use this hack, must to wait to bind success to network
            sdn_worker: TaskSwitcherBranch::new(SdnWorker::new(sdn), TaskType::Sdn),
            media_cluster: TaskSwitcherBranch::default(TaskType::MediaCluster),
            media_webrtc: TaskSwitcherBranch::new(MediaWorkerWebrtc::new(media.webrtc_addrs, media.ice_lite), TaskType::MediaWebrtc),
            switcher: TaskSwitcher::new(3),
            queue: DynamicDeque::from([Output::Net(Owner::Sdn, BackendOutgoing::UdpListen { addr: sdn_udp_addr, reuse: true })]),
            timer: TimePivot::build(),
        }
    }

    pub fn tasks(&self) -> usize {
        self.sdn_worker.tasks() + self.sdn_worker.tasks()
    }

    pub fn on_tick(&mut self, now: Instant) {
        let s = &mut self.switcher;
        let now_ms = self.timer.timestamp_ms(now);
        self.sdn_worker.input(s).on_tick(now_ms);
        self.media_cluster.input(s).on_tick(now);
        self.media_webrtc.input(s).on_tick(now);
    }

    pub fn on_event(&mut self, now: Instant, input: Input) {
        match input {
            Input::ExtRpc(req_id, req) => self.process_rpc(now, req_id, req),
            Input::ExtSdn(ext) => {
                let now_ms = self.timer.timestamp_ms(now);
                self.sdn_worker.input(&mut self.switcher).on_event(now_ms, SdnWorkerInput::Ext(ext));
            }
            Input::Net(owner, event) => match owner {
                Owner::Sdn => {
                    let now_ms = self.timer.timestamp_ms(now);
                    match event {
                        BackendIncoming::UdpPacket { slot: _, from, data } => {
                            self.sdn_worker.input(&mut self.switcher).on_event(now_ms, SdnWorkerInput::Net(NetInput::UdpPacket(from, data)));
                        }
                        BackendIncoming::UdpListenResult { bind: _, result } => {
                            let (addr, slot) = result.expect("Should listen ok");
                            log::info!("[MediaServerWorker] sdn listen success on {addr}, slot {slot}");
                            self.sdn_slot = slot;
                        }
                    }
                }
                Owner::MediaWebrtc => {
                    self.media_webrtc.input(&mut self.switcher).on_event(now, transport_webrtc::GroupInput::Net(event));
                }
            },
            Input::Bus(event) => {
                let now_ms = self.timer.timestamp_ms(now);
                self.sdn_worker.input(&mut self.switcher).on_event(now_ms, SdnWorkerInput::Bus(event));
            }
        }
    }

    pub fn pop_output(&mut self, now: Instant) -> Option<Output> {
        if let Some(out) = self.queue.pop_front() {
            return Some(out);
        }
        while let Some(c) = self.switcher.current() {
            match c.try_into().ok()? {
                TaskType::Sdn => {
                    let now_ms = self.timer.timestamp_ms(now);
                    if let Some(out) = self.sdn_worker.pop_output(now_ms, &mut self.switcher) {
                        return Some(self.output_sdn(now, out));
                    }
                }
                TaskType::MediaCluster => {
                    if let Some(out) = self.media_cluster.pop_output(now, &mut self.switcher) {
                        return Some(self.output_cluster(now, out));
                    }
                }
                TaskType::MediaWebrtc => {
                    if let Some(out) = self.media_webrtc.pop_output(now, &mut self.switcher) {
                        return Some(self.output_webrtc(now, out));
                    }
                }
            }
        }
        None
    }

    pub fn shutdown(&mut self, now: Instant) {
        let now_ms = self.timer.timestamp_ms(now);
        self.sdn_worker.input(&mut self.switcher).on_event(now_ms, SdnWorkerInput::ShutdownRequest);
        self.media_cluster.input(&mut self.switcher).shutdown(now);
        self.media_webrtc.input(&mut self.switcher).shutdown(now);
    }
}

impl MediaServerWorker {
    fn output_sdn(&mut self, now: Instant, out: SdnWorkerOutput<UserData, SC, SE, TC, TW>) -> Output {
        match out {
            SdnWorkerOutput::Ext(out) => Output::ExtSdn(out),
            SdnWorkerOutput::ExtWorker(out) => match out {
                SdnExtOut::FeaturesEvent(room, event) => {
                    self.media_cluster.input(&mut self.switcher).on_sdn_event(now, room, event);
                    Output::Continue
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

    fn output_cluster(&mut self, now: Instant, out: cluster::Output<MediaClusterOwner>) -> Output {
        match out {
            cluster::Output::Sdn(userdata, control) => {
                let now_ms = self.timer.timestamp_ms(now);
                self.sdn_worker
                    .input(&mut self.switcher)
                    .on_event(now_ms, SdnWorkerInput::ExtWorker(SdnExtIn::FeaturesControl(userdata, control)));
                Output::Continue
            }
            cluster::Output::Endpoint(owners, event) => {
                for owner in owners {
                    match owner {
                        MediaClusterOwner::Webrtc(owner) => {
                            self.media_webrtc.input(&mut self.switcher).on_event(now, transport_webrtc::GroupInput::Cluster(owner, event.clone()));
                        }
                    }
                }
                Output::Continue
            }
            cluster::Output::Continue => Output::Continue,
        }
    }

    fn output_webrtc(&mut self, now: Instant, out: transport_webrtc::GroupOutput) -> Output {
        match out {
            transport_webrtc::GroupOutput::Net(out) => Output::Net(Owner::MediaWebrtc, out),
            transport_webrtc::GroupOutput::Cluster(owner, room, control) => {
                self.media_cluster.input(&mut self.switcher).on_endpoint_control(now, owner.into(), room, control);
                Output::Continue
            }
            transport_webrtc::GroupOutput::Ext(owner, ext) => match ext {
                transport_webrtc::ExtOut::RemoteIce(req_id, variant, res) => match variant {
                    transport_webrtc::Variant::Whip => Output::ExtRpc(req_id, RpcRes::Whip(whip::RpcRes::RemoteIce(res.map(|_| WhipRemoteIceRes {})))),
                    transport_webrtc::Variant::Whep => Output::ExtRpc(req_id, RpcRes::Whep(whep::RpcRes::RemoteIce(res.map(|_| WhepRemoteIceRes {})))),
                    transport_webrtc::Variant::Webrtc => Output::ExtRpc(req_id, RpcRes::Webrtc(webrtc::RpcRes::RemoteIce(res.map(|added| RemoteIceResponse { added })))),
                },
                transport_webrtc::ExtOut::RestartIce(req_id, _, res) => Output::ExtRpc(
                    req_id,
                    RpcRes::Webrtc(webrtc::RpcRes::RestartIce(res.map(|(ice_lite, sdp)| {
                        (
                            owner.index(),
                            ConnectResponse {
                                conn_id: "".to_string(),
                                sdp,
                                ice_lite,
                            },
                        )
                    }))),
                ),
            },
            transport_webrtc::GroupOutput::Shutdown(_owner) => Output::Continue,
            transport_webrtc::GroupOutput::Continue => Output::Continue,
        }
    }
}

impl MediaServerWorker {
    fn process_rpc(&mut self, now: Instant, req_id: u64, req: RpcReq<usize>) {
        log::info!("[MediaServerWorker] incoming rpc req {req_id}");
        match req {
            RpcReq::Whip(req) => match req {
                whip::RpcReq::Connect(req) => match self
                    .media_webrtc
                    .input(&mut self.switcher)
                    .spawn(transport_webrtc::VariantParams::Whip(req.token.into(), "publisher".to_string().into()), &req.sdp)
                {
                    Ok((_ice_lite, sdp, conn_id)) => self.queue.push_back(Output::ExtRpc(req_id, RpcRes::Whip(whip::RpcRes::Connect(Ok(WhipConnectRes { conn_id, sdp }))))),
                    Err(e) => self.queue.push_back(Output::ExtRpc(req_id, RpcRes::Whip(whip::RpcRes::Connect(Err(e))))),
                },
                whip::RpcReq::RemoteIce(req) => {
                    log::info!("on rpc request {req_id}, whip::RpcReq::RemoteIce");
                    self.media_webrtc.input(&mut self.switcher).on_event(
                        now,
                        GroupInput::Ext(req.conn_id.into(), transport_webrtc::ExtIn::RemoteIce(req_id, transport_webrtc::Variant::Whip, vec![req.ice])),
                    );
                }
                whip::RpcReq::Delete(req) => {
                    //TODO check error instead of auto response ok
                    self.queue.push_back(Output::ExtRpc(req_id, RpcRes::Whip(whip::RpcRes::Delete(Ok(WhipDeleteRes {})))));
                    self.media_webrtc.input(&mut self.switcher).on_event(now, GroupInput::Close(req.conn_id.into()));
                }
            },
            RpcReq::Whep(req) => match req {
                whep::RpcReq::Connect(req) => {
                    let peer_id = format!("whep-{}", random::<u64>());
                    match self
                        .media_webrtc
                        .input(&mut self.switcher)
                        .spawn(transport_webrtc::VariantParams::Whep(req.token.into(), peer_id.into()), &req.sdp)
                    {
                        Ok((_ice_lite, sdp, conn_id)) => self.queue.push_back(Output::ExtRpc(req_id, RpcRes::Whep(whep::RpcRes::Connect(Ok(WhepConnectRes { conn_id, sdp }))))),
                        Err(e) => self.queue.push_back(Output::ExtRpc(req_id, RpcRes::Whep(whep::RpcRes::Connect(Err(e))))),
                    }
                }
                whep::RpcReq::RemoteIce(req) => {
                    log::info!("on rpc request {req_id}, whep::RpcReq::RemoteIce");
                    self.media_webrtc.input(&mut self.switcher).on_event(
                        now,
                        GroupInput::Ext(req.conn_id.into(), transport_webrtc::ExtIn::RemoteIce(req_id, transport_webrtc::Variant::Whep, vec![req.ice])),
                    );
                }
                whep::RpcReq::Delete(req) => {
                    //TODO check error instead of auto response ok
                    self.queue.push_back(Output::ExtRpc(req_id, RpcRes::Whep(whep::RpcRes::Delete(Ok(WhepDeleteRes {})))));
                    self.media_webrtc.input(&mut self.switcher).on_event(now, GroupInput::Close(req.conn_id.into()));
                }
            },
            RpcReq::Webrtc(req) => match req {
                webrtc::RpcReq::Connect(ip, token, user_agent, req) => match self.media_webrtc.input(&mut self.switcher).spawn(VariantParams::Webrtc(ip, token, user_agent, req.clone()), &req.sdp) {
                    Ok((ice_lite, sdp, conn_id)) => self.queue.push_back(Output::ExtRpc(
                        req_id,
                        RpcRes::Webrtc(webrtc::RpcRes::Connect(Ok((
                            conn_id.into(),
                            ConnectResponse {
                                conn_id: "".to_string(),
                                sdp,
                                ice_lite,
                            },
                        )))),
                    )),
                    Err(e) => self.queue.push_back(Output::ExtRpc(req_id, RpcRes::Webrtc(webrtc::RpcRes::Connect(Err(e))))),
                },
                webrtc::RpcReq::RemoteIce(conn, ice) => {
                    log::info!("on rpc request {req_id}, webrtc::RpcReq::RemoteIce");
                    self.media_webrtc.input(&mut self.switcher).on_event(
                        now,
                        GroupInput::Ext(conn.into(), transport_webrtc::ExtIn::RemoteIce(req_id, transport_webrtc::Variant::Webrtc, ice.candidates)),
                    );
                }
                webrtc::RpcReq::RestartIce(conn, ip, token, user_agent, req) => {
                    log::info!("on rpc request {req_id}, webrtc::RpcReq::RestartIce");
                    self.media_webrtc.input(&mut self.switcher).on_event(
                        now,
                        GroupInput::Ext(conn.into(), transport_webrtc::ExtIn::RestartIce(req_id, transport_webrtc::Variant::Webrtc, ip, token, user_agent, req)),
                    );
                }
                webrtc::RpcReq::Delete(_) => todo!(),
            },
        }
    }
}
