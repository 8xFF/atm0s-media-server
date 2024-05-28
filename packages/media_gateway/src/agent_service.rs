use std::{collections::VecDeque, fmt::Debug};

use atm0s_sdn::{
    base::{NetOutgoingMeta, Service, ServiceBuilder, ServiceCtx, ServiceInput, ServiceOutput, ServiceSharedInput, ServiceWorker, ServiceWorkerCtx, ServiceWorkerInput, ServiceWorkerOutput},
    features::{data, FeaturesControl, FeaturesEvent},
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

use crate::{ServiceKind, AGENT_SERVICE_ID, AGENT_SERVICE_NAME, DATA_PORT, STORE_SERVICE_ID};

#[derive(Debug, Clone)]
pub enum Control {
    Stats(Vec<(ServiceKind, u32)>),
}

#[derive(Debug, Clone)]
pub enum Event {}

pub struct GatewayAgentService<UserData, SC, SE, TC, TW> {
    output: Option<ServiceOutput<UserData, FeaturesControl, SE, TW>>,
    seq: u16,
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
}

impl<UserData: Copy, SC, SE, TC, TW> GatewayAgentService<UserData, SC, SE, TC, TW>
where
    SC: From<Control> + TryInto<Control>,
    SE: From<Event> + TryInto<Event>,
{
    pub fn new() -> Self {
        Self {
            output: None,
            seq: 0,
            _tmp: std::marker::PhantomData,
        }
    }
}

impl<UserData: Copy + Eq, SC, SE, TC, TW> Service<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for GatewayAgentService<UserData, SC, SE, TC, TW>
where
    SC: From<Control> + TryInto<Control>,
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
                        cpu: 0,    //TODO
                        memory: 0, //TODO
                        disk: 0,   //TODO
                        webrtc: Some(ServiceStats { active: true, live: 0, max: 100 }),
                        origin: Some(Origin::Media(MediaOrigin {})),
                    })),
                }
                .encode_to_vec();
                log::info!("[GatewayAgent] broadcast ping to zone gateways");
                self.output = Some(ServiceOutput::FeatureControl(data::Control::DataSendRule(DATA_PORT, rule, meta, data.into()).into()));
            }
            ServiceSharedInput::Connection(_) => {}
        }
    }

    fn on_input(&mut self, _ctx: &ServiceCtx, _now: u64, _input: ServiceInput<UserData, FeaturesEvent, SC, TC>) {}

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
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
}

impl<UserData, SC, SE, TC, TW> GatewayAgentServiceBuilder<UserData, SC, SE, TC, TW> {
    pub fn new() -> Self {
        Self { _tmp: std::marker::PhantomData }
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
        Box::new(GatewayAgentService::new())
    }

    fn create_worker(&self) -> Box<dyn ServiceWorker<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW>> {
        Box::new(GatewayAgentServiceWorker { queue: Default::default() })
    }
}
