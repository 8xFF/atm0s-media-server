use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
};

use atm0s_sdn::{
    base::{NetOutgoingMeta, Service, ServiceBuilder, ServiceCtx, ServiceInput, ServiceOutput, ServiceSharedInput, ServiceWorker, ServiceWorkerCtx, ServiceWorkerInput, ServiceWorkerOutput},
    features::{data, FeaturesControl, FeaturesEvent},
    sans_io_runtime::return_if_err,
    RouteRule, ServiceBroadcastLevel,
};
use media_server_protocol::protobuf::{
    self,
    cluster_gateway::{
        gateway_event,
        ping_event::{MediaOrigin, Origin, ServiceStats},
    },
};
use prost::Message as _;

use crate::{NodeMetrics, ServiceKind, AGENT_SERVICE_ID, AGENT_SERVICE_NAME, DATA_PORT, STORE_SERVICE_ID};

struct ServiceWorkersStats {
    max: u32,
    workers: HashMap<u16, u32>,
}

impl From<&ServiceWorkersStats> for ServiceStats {
    fn from(value: &ServiceWorkersStats) -> Self {
        Self {
            live: value.workers.values().sum(),
            max: value.max,
            active: true, //TODO how to update this? maybe with graceful-shutdown
        }
    }
}

#[derive(Debug, Clone)]
pub enum Control {
    NodeStats(NodeMetrics),
    WorkerUsage(ServiceKind, u16, u32),
}

#[derive(Debug, Clone)]
pub enum Event {}

pub struct GatewayAgentService<UserData, SC, SE, TC, TW> {
    output: Option<ServiceOutput<UserData, FeaturesControl, SE, TW>>,
    seq: u16,
    node: NodeMetrics,
    services: HashMap<ServiceKind, ServiceWorkersStats>,
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
}

impl<UserData, SC, SE, TC, TW> GatewayAgentService<UserData, SC, SE, TC, TW> {
    pub fn new(max: HashMap<ServiceKind, u32>) -> Self {
        Self {
            output: None,
            seq: 0,
            node: Default::default(),
            services: HashMap::from_iter(max.into_iter().map(|(k, v)| (k, ServiceWorkersStats { max: v, workers: HashMap::new() }))),
            _tmp: std::marker::PhantomData,
        }
    }
}

impl<UserData: Copy + Eq, SC, SE, TC, TW> Service<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for GatewayAgentService<UserData, SC, SE, TC, TW>
where
    SC: From<Control> + TryInto<Control> + Debug,
    SE: From<Event> + TryInto<Event>,
{
    fn service_id(&self) -> u8 {
        AGENT_SERVICE_ID
    }

    fn service_name(&self) -> &str {
        AGENT_SERVICE_NAME
    }

    fn on_shared_input<'a>(&mut self, _ctx: &ServiceCtx, _now: u64, input: ServiceSharedInput) {
        match input {
            ServiceSharedInput::Tick(_) => {
                let rule = RouteRule::ToServices(STORE_SERVICE_ID, ServiceBroadcastLevel::Group, self.seq);
                self.seq += 1;
                let mut meta = NetOutgoingMeta::secure();
                meta.source = true;
                let data = protobuf::cluster_gateway::GatewayEvent {
                    event: Some(gateway_event::Event::Ping(protobuf::cluster_gateway::PingEvent {
                        cpu: self.node.cpu as u32,
                        memory: self.node.memory as u32,
                        disk: self.node.disk as u32,
                        webrtc: self.services.get(&ServiceKind::Webrtc).map(|s| s.into()),
                        rtpengine: self.services.get(&ServiceKind::RtpEngine).map(|s| s.into()),
                        origin: Some(Origin::Media(MediaOrigin {})),
                    })),
                }
                .encode_to_vec();
                log::debug!("[GatewayAgent] broadcast ping to zone gateways");
                self.output = Some(ServiceOutput::FeatureControl(data::Control::DataSendRule(DATA_PORT, rule, meta, data).into()));
            }
            ServiceSharedInput::Connection(_) => {}
        }
    }

    fn on_input(&mut self, _ctx: &ServiceCtx, _now: u64, input: ServiceInput<UserData, FeaturesEvent, SC, TC>) {
        match input {
            ServiceInput::Control(_, control) => match return_if_err!(control.try_into()) {
                Control::NodeStats(metrics) => {
                    log::debug!("[GatewayAgentService] node metrics {:?}", metrics);
                    self.node = metrics;
                }
                Control::WorkerUsage(kind, worker, live) => {
                    log::debug!("[GatewayAgentService] worker {worker} live {live}");
                    if let Some(service) = self.services.get_mut(&kind) {
                        service.workers.insert(worker, live);
                    }
                }
            },
            ServiceInput::FromWorker(_) => {}
            ServiceInput::FeatureEvent(_) => {}
        }
    }

    fn pop_output2(&mut self, _now: u64) -> Option<ServiceOutput<UserData, FeaturesControl, SE, TW>> {
        self.output.take()
    }
}

pub struct GatewayAgentServiceWorker<UserData, SC, SE, TC> {
    queue: VecDeque<ServiceWorkerOutput<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC>>,
}

impl<UserData, SC, SE, TC, TW> ServiceWorker<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for GatewayAgentServiceWorker<UserData, SC, SE, TC> {
    fn service_id(&self) -> u8 {
        AGENT_SERVICE_ID
    }

    fn service_name(&self) -> &str {
        AGENT_SERVICE_NAME
    }

    fn on_tick(&mut self, _ctx: &ServiceWorkerCtx, _now: u64, _tick_count: u64) {}

    fn on_input(&mut self, _ctx: &ServiceWorkerCtx, _now: u64, input: ServiceWorkerInput<UserData, FeaturesEvent, SC, TW>) {
        match input {
            ServiceWorkerInput::Control(owner, control) => self.queue.push_back(ServiceWorkerOutput::ForwardControlToController(owner, control)),
            ServiceWorkerInput::FromController(_) => {}
            ServiceWorkerInput::FeatureEvent(event) => self.queue.push_back(ServiceWorkerOutput::ForwardFeatureEventToController(event)),
        }
    }

    fn pop_output2(&mut self, _now: u64) -> Option<ServiceWorkerOutput<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC>> {
        self.queue.pop_front()
    }
}

pub struct GatewayAgentServiceBuilder<UserData, SC, SE, TC, TW> {
    max: HashMap<ServiceKind, u32>,
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
}

impl<UserData, SC, SE, TC, TW> GatewayAgentServiceBuilder<UserData, SC, SE, TC, TW> {
    pub fn new(max: HashMap<ServiceKind, u32>) -> Self {
        Self { max, _tmp: std::marker::PhantomData }
    }
}

impl<UserData, SC, SE, TC, TW> ServiceBuilder<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for GatewayAgentServiceBuilder<UserData, SC, SE, TC, TW>
where
    UserData: 'static + Debug + Send + Sync + Copy + Eq,
    SC: 'static + Debug + Send + Sync + From<Control> + TryInto<Control>,
    SE: 'static + Debug + Send + Sync + From<Event> + TryInto<Event>,
    TC: 'static + Debug + Send + Sync,
    TW: 'static + Debug + Send + Sync,
{
    fn service_id(&self) -> u8 {
        AGENT_SERVICE_ID
    }

    fn service_name(&self) -> &str {
        AGENT_SERVICE_NAME
    }

    fn discoverable(&self) -> bool {
        false
    }

    fn create(&self) -> Box<dyn Service<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW>> {
        Box::new(GatewayAgentService::new(self.max.clone()))
    }

    fn create_worker(&self) -> Box<dyn ServiceWorker<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW>> {
        Box::new(GatewayAgentServiceWorker { queue: Default::default() })
    }
}
