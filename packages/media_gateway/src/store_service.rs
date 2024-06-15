use std::{collections::VecDeque, fmt::Debug};

use atm0s_sdn::{
    base::{
        NetIncomingMeta, NetOutgoingMeta, Service, ServiceBuilder, ServiceCtx, ServiceInput, ServiceOutput, ServiceSharedInput, ServiceWorker, ServiceWorkerCtx, ServiceWorkerInput,
        ServiceWorkerOutput,
    },
    features::{data, FeaturesControl, FeaturesEvent},
    RouteRule, ServiceBroadcastLevel,
};
use media_server_protocol::protobuf::{
    self,
    cluster_gateway::{gateway_event, ping_event::gateway_origin::Location},
};
use prost::Message as _;

use crate::{
    store::{GatewayStore, PingEvent},
    NodeMetrics, ServiceKind, DATA_PORT, STORE_SERVICE_ID, STORE_SERVICE_NAME,
};

#[derive(Debug, Clone)]
pub enum Control {
    NodeStats(NodeMetrics),
    FindNodeReq(u64, ServiceKind, Option<Location>),
    GetMediaStats,
}

#[derive(Debug, Clone)]
pub enum Event {
    MediaStats(u32, u32),
    FindNodeRes(u64, Option<u32>),
}

pub struct GatewayStoreService<UserData, SC, SE, TC, TW> {
    queue: VecDeque<ServiceOutput<UserData, FeaturesControl, SE, TW>>,
    store: GatewayStore,
    seq: u16,
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
}

impl<UserData: Copy, SC, SE, TC, TW> GatewayStoreService<UserData, SC, SE, TC, TW>
where
    SC: From<Control> + TryInto<Control>,
    SE: From<Event> + TryInto<Event>,
{
    pub fn new(zone: u32, lat: f32, lon: f32, max_cpu: u8, max_memory: u8, max_disk: u8) -> Self {
        Self {
            store: GatewayStore::new(zone, Location { lat, lon }, max_cpu, max_memory, max_disk),
            queue: VecDeque::from([ServiceOutput::FeatureControl(data::Control::DataListen(DATA_PORT).into())]),
            seq: 0,
            _tmp: std::marker::PhantomData,
        }
    }

    fn handle_event(&mut self, now: u64, _port: u16, meta: NetIncomingMeta, data: &[u8]) -> Option<()> {
        let from = meta.source?;
        let req = protobuf::cluster_gateway::GatewayEvent::decode(data).ok()?;
        let event = req.event?;
        match event {
            gateway_event::Event::Ping(ping) => {
                let origin = ping.origin?;
                self.store.on_ping(
                    now,
                    from,
                    PingEvent {
                        cpu: ping.cpu as u8,
                        memory: ping.memory as u8,
                        disk: ping.disk as u8,
                        origin,
                        webrtc: ping.webrtc,
                    },
                )
            }
        }

        Some(())
    }
}

impl<UserData: Copy + Eq + Debug, SC, SE, TC, TW> Service<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for GatewayStoreService<UserData, SC, SE, TC, TW>
where
    SC: From<Control> + TryInto<Control> + Debug,
    SE: From<Event> + TryInto<Event>,
    TC: Debug,
{
    fn service_id(&self) -> u8 {
        STORE_SERVICE_ID
    }

    fn service_name(&self) -> &str {
        STORE_SERVICE_NAME
    }

    fn on_shared_input<'a>(&mut self, _ctx: &ServiceCtx, now: u64, input: ServiceSharedInput) {
        match input {
            ServiceSharedInput::Tick(_) => {
                self.store.on_tick(now);
                if let Some(ping) = self.store.pop_output() {
                    let rule = RouteRule::ToServices(STORE_SERVICE_ID, ServiceBroadcastLevel::Global, self.seq);
                    self.seq += 1;
                    let mut meta = NetOutgoingMeta::secure();
                    meta.source = true;
                    let data = protobuf::cluster_gateway::GatewayEvent {
                        event: Some(gateway_event::Event::Ping(protobuf::cluster_gateway::PingEvent {
                            cpu: ping.cpu as u32,
                            memory: ping.memory as u32,
                            disk: ping.disk as u32,
                            webrtc: ping.webrtc,
                            origin: Some(ping.origin),
                        })),
                    }
                    .encode_to_vec();
                    self.queue.push_back(ServiceOutput::FeatureControl(data::Control::DataSendRule(DATA_PORT, rule, meta, data).into()));
                }
            }
            ServiceSharedInput::Connection(_) => {}
        }
    }

    fn on_input(&mut self, _ctx: &ServiceCtx, now: u64, input: ServiceInput<UserData, FeaturesEvent, SC, TC>) {
        match input {
            ServiceInput::FeatureEvent(FeaturesEvent::Data(data::Event::Recv(port, meta, data))) => {
                self.handle_event(now, port, meta, &data);
            }
            ServiceInput::Control(actor, control) => {
                if let Ok(control) = control.try_into() {
                    match control {
                        Control::FindNodeReq(req_id, kind, location) => {
                            let out = self.store.best_for(kind, location);
                            self.queue.push_back(ServiceOutput::Event(actor, Event::FindNodeRes(req_id, out).into()));
                        }
                        Control::NodeStats(metrics) => {
                            log::debug!("[GatewayStoreService] node metrics {:?}", metrics);
                            self.store.on_node_metrics(now, metrics);
                        }
                        Control::GetMediaStats => {
                            if let Some(stats) = self.store.local_stats() {
                                self.queue.push_back(ServiceOutput::Event(actor, Event::MediaStats(stats.live, stats.max).into()));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn pop_output2(&mut self, _now: u64) -> Option<ServiceOutput<UserData, FeaturesControl, SE, TW>> {
        self.queue.pop_front()
    }
}

pub struct GatewayStoreServiceWorker<UserData, SC, SE, TC> {
    queue: VecDeque<ServiceWorkerOutput<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC>>,
}

impl<UserData, SC, SE, TC, TW> ServiceWorker<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for GatewayStoreServiceWorker<UserData, SC, SE, TC> {
    fn service_id(&self) -> u8 {
        STORE_SERVICE_ID
    }

    fn service_name(&self) -> &str {
        STORE_SERVICE_NAME
    }

    fn on_tick(&mut self, _ctx: &ServiceWorkerCtx, _now: u64, _tick_count: u64) {}

    fn on_input(&mut self, _ctx: &ServiceWorkerCtx, _now: u64, input: ServiceWorkerInput<UserData, FeaturesEvent, SC, TW>) {
        match input {
            ServiceWorkerInput::Control(owner, control) => self.queue.push_back(ServiceWorkerOutput::ForwardControlToController(owner, control)),
            ServiceWorkerInput::FromController(_) => {}
            ServiceWorkerInput::FeatureEvent(event) => {
                log::info!("forward event to controller");
                self.queue.push_back(ServiceWorkerOutput::ForwardFeatureEventToController(event))
            }
        }
    }

    fn pop_output2(&mut self, _now: u64) -> Option<ServiceWorkerOutput<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC>> {
        self.queue.pop_front()
    }
}

pub struct GatewayStoreServiceBuilder<UserData, SC, SE, TC, TW> {
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
    zone: u32,
    lat: f32,
    lon: f32,
    max_memory: u8,
    max_disk: u8,
    max_cpu: u8,
}

impl<UserData, SC, SE, TC, TW> GatewayStoreServiceBuilder<UserData, SC, SE, TC, TW> {
    pub fn new(zone: u32, lat: f32, lon: f32, max_cpu: u8, max_memory: u8, max_disk: u8) -> Self {
        Self {
            zone,
            lat,
            lon,
            _tmp: std::marker::PhantomData,
            max_cpu,
            max_memory,
            max_disk,
        }
    }
}

impl<UserData, SC, SE, TC, TW> ServiceBuilder<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for GatewayStoreServiceBuilder<UserData, SC, SE, TC, TW>
where
    UserData: 'static + Debug + Send + Sync + Copy + Eq,
    SC: 'static + Debug + Send + Sync + From<Control> + TryInto<Control>,
    SE: 'static + Debug + Send + Sync + From<Event> + TryInto<Event>,
    TC: 'static + Debug + Send + Sync,
    TW: 'static + Debug + Send + Sync,
{
    fn service_id(&self) -> u8 {
        STORE_SERVICE_ID
    }

    fn service_name(&self) -> &str {
        STORE_SERVICE_NAME
    }

    fn discoverable(&self) -> bool {
        true
    }

    fn create(&self) -> Box<dyn Service<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW>> {
        Box::new(GatewayStoreService::new(self.zone, self.lat, self.lon, self.max_cpu, self.max_memory, self.max_disk))
    }

    fn create_worker(&self) -> Box<dyn ServiceWorker<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW>> {
        Box::new(GatewayStoreServiceWorker { queue: Default::default() })
    }
}
