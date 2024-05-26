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
    cluster_gateway::{gateway_event, ping_event::gateway_origin::Location, GatewayEvent},
};
use prost::Message as _;

use crate::store::{GatewayStore, PingEvent};

pub const DATA_PORT: u16 = 10001;
pub const SERVICE_ID: u8 = 101;
pub const SERVICE_NAME: &str = "gateway_connect";

#[derive(Debug, Clone)]
pub enum Control {}

#[derive(Debug, Clone)]
pub enum Event {}

pub struct GatewayService<UserData, SC, SE, TC, TW> {
    queue: VecDeque<ServiceOutput<UserData, FeaturesControl, SE, TW>>,
    store: GatewayStore,
    seq: u16,
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
}

impl<UserData: Copy, SC, SE, TC, TW> GatewayService<UserData, SC, SE, TC, TW>
where
    SC: From<Control> + TryInto<Control>,
    SE: From<Event> + TryInto<Event>,
{
    pub fn new(lat: f32, lon: f32) -> Self {
        Self {
            store: GatewayStore::new(Location { lat, lon }),
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

impl<UserData: Copy + Eq, SC, SE, TC, TW> Service<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for GatewayService<UserData, SC, SE, TC, TW>
where
    SC: From<Control> + TryInto<Control>,
    SE: From<Event> + TryInto<Event>,
{
    fn service_id(&self) -> u8 {
        SERVICE_ID
    }

    fn service_name(&self) -> &str {
        SERVICE_NAME
    }

    fn on_shared_input<'a>(&mut self, ctx: &ServiceCtx, now: u64, input: ServiceSharedInput) {
        match input {
            ServiceSharedInput::Tick(_) => {
                self.store.on_tick(now);
                if let Some(ping) = self.store.pop_output() {
                    let rule = RouteRule::ToServices(SERVICE_ID, ServiceBroadcastLevel::Global, self.seq);
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
                    self.queue
                        .push_back(ServiceOutput::FeatureControl(data::Control::DataSendRule(DATA_PORT, rule, meta, data.into()).into()));
                }
            }
            ServiceSharedInput::Connection(_) => {}
        }
    }

    fn on_input(&mut self, _ctx: &ServiceCtx, now: u64, input: ServiceInput<UserData, FeaturesEvent, SC, TC>) {
        if let ServiceInput::FeatureEvent(FeaturesEvent::Data(data::Event::Recv(port, meta, data))) = input {
            self.handle_event(now, port, meta, &data);
        }
    }

    fn pop_output2(&mut self, _now: u64) -> Option<ServiceOutput<UserData, FeaturesControl, SE, TW>> {
        self.queue.pop_front()
    }
}

pub struct GatewayServiceWorker<UserData, SC, SE, TC> {
    queue: VecDeque<ServiceWorkerOutput<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC>>,
}

impl<UserData, SC, SE, TC, TW> ServiceWorker<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for GatewayServiceWorker<UserData, SC, SE, TC> {
    fn service_id(&self) -> u8 {
        SERVICE_ID
    }

    fn service_name(&self) -> &str {
        SERVICE_NAME
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

pub struct GatewayServiceBuilder<UserData, SC, SE, TC, TW> {
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
    lat: f32,
    lon: f32,
}

impl<UserData, SC, SE, TC, TW> GatewayServiceBuilder<UserData, SC, SE, TC, TW> {
    pub fn new(lat: f32, lon: f32) -> Self {
        Self {
            lat,
            lon,
            _tmp: std::marker::PhantomData,
        }
    }
}

impl<UserData, SC, SE, TC, TW> ServiceBuilder<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for GatewayServiceBuilder<UserData, SC, SE, TC, TW>
where
    UserData: 'static + Debug + Send + Sync + Copy + Eq,
    SC: 'static + Debug + Send + Sync + From<Control> + TryInto<Control>,
    SE: 'static + Debug + Send + Sync + From<Event> + TryInto<Event>,
    TC: 'static + Debug + Send + Sync,
    TW: 'static + Debug + Send + Sync,
{
    fn service_id(&self) -> u8 {
        SERVICE_ID
    }

    fn service_name(&self) -> &str {
        SERVICE_NAME
    }

    fn discoverable(&self) -> bool {
        false
    }

    fn create(&self) -> Box<dyn Service<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW>> {
        Box::new(GatewayService::new(self.lat, self.lon))
    }

    fn create_worker(&self) -> Box<dyn ServiceWorker<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW>> {
        Box::new(GatewayServiceWorker { queue: Default::default() })
    }
}
