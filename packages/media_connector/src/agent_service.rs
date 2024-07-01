use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
};

use atm0s_sdn::{
    base::{
        NetOutgoingMeta, Service, ServiceBuilder, ServiceControlActor, ServiceCtx, ServiceInput, ServiceOutput, ServiceSharedInput, ServiceWorker, ServiceWorkerCtx, ServiceWorkerInput,
        ServiceWorkerOutput,
    },
    features::{data, FeaturesControl, FeaturesEvent},
    RouteRule,
};

use media_server_protocol::protobuf::cluster_connector::{connector_request, connector_response, ConnectorRequest, ConnectorResponse};
use prost::Message;

use crate::{msg_queue::MessageQueue, AGENT_SERVICE_ID, AGENT_SERVICE_NAME, DATA_PORT, HANDLER_SERVICE_ID};

#[derive(Debug, Clone)]
pub enum Control {
    Sub,
    Request(u64, connector_request::Request),
}

#[derive(Debug, Clone)]
pub enum Event {
    Response(connector_response::Response),
    Stats { queue: usize, inflight: usize, acked: usize },
}

pub struct ConnectorAgentService<UserData, SC, SE, TC, TW> {
    req_id_seed: u64,
    req_data: HashMap<u64, ServiceControlActor<UserData>>,
    subscriber: Option<ServiceControlActor<UserData>>,
    msg_queue: MessageQueue<ConnectorRequest, 1024>,
    queue: VecDeque<ServiceOutput<UserData, FeaturesControl, SE, TW>>,
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
}

impl<UserData, SC, SE, TC, TW> ConnectorAgentService<UserData, SC, SE, TC, TW> {
    pub fn new() -> Self {
        Self {
            req_id_seed: 0,
            req_data: HashMap::new(),
            subscriber: None,
            queue: VecDeque::from([ServiceOutput::FeatureControl(data::Control::DataListen(DATA_PORT).into())]),
            msg_queue: MessageQueue::default(),
            _tmp: std::marker::PhantomData,
        }
    }
}

impl<UserData: Copy + Eq, SC, SE, TC, TW> Service<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for ConnectorAgentService<UserData, SC, SE, TC, TW>
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
                if let Some(subscriber) = self.subscriber {
                    self.queue.push_back(ServiceOutput::Event(
                        subscriber,
                        Event::Stats {
                            queue: self.msg_queue.waits(),
                            inflight: self.msg_queue.inflight(),
                            acked: self.msg_queue.acked(),
                        }
                        .into(),
                    ));
                }
            }
            ServiceSharedInput::Connection(_) => {}
        }
    }

    fn on_input(&mut self, _ctx: &ServiceCtx, _now: u64, input: ServiceInput<UserData, FeaturesEvent, SC, TC>) {
        match input {
            ServiceInput::Control(owner, control) => {
                if let Ok(control) = control.try_into() {
                    match control {
                        Control::Request(ts, request) => {
                            let req_id = self.req_id_seed;
                            self.req_id_seed += 1;
                            self.req_data.insert(req_id, owner);
                            let req = ConnectorRequest { req_id, ts, request: Some(request) };
                            log::info!("[ConnectorAgent] push msg to queue {:?}", req);
                            self.msg_queue.push(req);
                        }
                        Control::Sub => {
                            self.subscriber = Some(owner);
                        }
                    }
                }
            }
            ServiceInput::FromWorker(_data) => {}
            ServiceInput::FeatureEvent(FeaturesEvent::Data(event)) => match event {
                data::Event::Pong(_, _) => {}
                data::Event::Recv(_port, _meta, buf) => match ConnectorResponse::decode(buf.as_slice()) {
                    Ok(msg) => {
                        if let Some(actor) = self.req_data.remove(&msg.req_id) {
                            log::info!("[ConnectorAgent] on msg response {:?}", msg);
                            self.msg_queue.on_ack(msg.req_id);
                            if let Some(res) = msg.response {
                                self.queue.push_back(ServiceOutput::Event(actor, Event::Response(res).into()));
                            }
                        } else {
                            log::warn!("[ConnectorAgent] missing info for msg response {:?}", msg);
                        }
                    }
                    Err(er) => {
                        log::error!("[ConnectorAgent] decode data error {}", er);
                    }
                },
            },
            _ => {}
        }
    }

    fn pop_output2(&mut self, now: u64) -> Option<ServiceOutput<UserData, FeaturesControl, SE, TW>> {
        if let Some(out) = self.queue.pop_front() {
            return Some(out);
        }
        let out = self.msg_queue.pop(now)?;
        let buf = out.encode_to_vec();
        let mut meta = NetOutgoingMeta::secure();
        meta.source = true;
        log::info!("[ConnectorAgent] send msg to net {:?}", out);
        Some(ServiceOutput::FeatureControl(
            data::Control::DataSendRule(DATA_PORT, RouteRule::ToService(HANDLER_SERVICE_ID), meta, buf).into(),
        ))
    }
}

pub struct ConnectorAgentServiceWorker<UserData, SC, SE, TC> {
    queue: VecDeque<ServiceWorkerOutput<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC>>,
}

impl<UserData, SC, SE, TC, TW> ServiceWorker<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for ConnectorAgentServiceWorker<UserData, SC, SE, TC> {
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

pub struct ConnectorAgentServiceBuilder<UserData, SC, SE, TC, TW> {
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
}

impl<UserData, SC, SE, TC, TW> ConnectorAgentServiceBuilder<UserData, SC, SE, TC, TW> {
    pub fn new() -> Self {
        Self { _tmp: std::marker::PhantomData }
    }
}

impl<UserData, SC, SE, TC, TW> ServiceBuilder<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for ConnectorAgentServiceBuilder<UserData, SC, SE, TC, TW>
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
        Box::new(ConnectorAgentService::new())
    }

    fn create_worker(&self) -> Box<dyn ServiceWorker<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW>> {
        Box::new(ConnectorAgentServiceWorker { queue: Default::default() })
    }
}

impl crate::msg_queue::Message for ConnectorRequest {
    fn msg_id(&self) -> u64 {
        self.req_id
    }
}
