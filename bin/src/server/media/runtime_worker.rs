use std::{collections::VecDeque, sync::Arc, time::Instant};

use atm0s_sdn::{
    secure::{HandshakeBuilderXDA, StaticKeyAuthorization},
    services::visualization,
    ControllerPlaneCfg, DataPlaneCfg, DataWorkerHistory, SdnExtIn, SdnExtOut, SdnWorkerBusEvent,
};
use media_server_protocol::transport::{RpcReq, RpcRes};
use media_server_runner::{Input as WorkerInput, MediaConfig, MediaServerWorker, Output as WorkerOutput, Owner, SdnConfig, UserData, SC, SE, TC, TW};
use media_server_secure::MediaEdgeSecure;
use rand::rngs::OsRng;
use sans_io_runtime::{BusChannelControl, BusControl, BusEvent, WorkerInner, WorkerInnerInput, WorkerInnerOutput};

use crate::NodeConfig;

#[derive(Debug, Clone)]
pub enum ExtIn {
    Sdn(SdnExtIn<UserData, SC>),
    Rpc(u64, RpcReq<usize>),
}

#[derive(Debug, Clone)]
pub enum ExtOut {
    Rpc(u64, u16, RpcRes<usize>),
    Sdn(SdnExtOut<UserData, SE>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Controller,
    Worker(u16),
}
type Event = SdnWorkerBusEvent<UserData, SC, SE, TC, TW>;
pub struct ICfg<ES> {
    pub controller: bool,
    pub node: NodeConfig,
    pub media: MediaConfig<ES>,
}
type SCfg = ();

type Input = WorkerInnerInput<Owner, ExtIn, Channel, Event>;
type Output = WorkerInnerOutput<Owner, ExtOut, Channel, Event, SCfg>;

pub struct MediaRuntimeWorker<ES: 'static + MediaEdgeSecure> {
    index: u16,
    worker: MediaServerWorker<ES>,
    queue: VecDeque<Output>,
}

impl<ES: 'static + MediaEdgeSecure> WorkerInner<Owner, ExtIn, ExtOut, Channel, Event, ICfg<ES>, SCfg> for MediaRuntimeWorker<ES> {
    fn build(index: u16, cfg: ICfg<ES>) -> Self {
        let sdn_config = SdnConfig {
            node_id: cfg.node.node_id,
            controller: if cfg.controller {
                Some(ControllerPlaneCfg {
                    session: cfg.node.session,
                    authorization: Arc::new(StaticKeyAuthorization::new(&cfg.node.secret)),
                    handshake_builder: Arc::new(HandshakeBuilderXDA),
                    random: Box::new(OsRng::default()),
                    services: vec![Arc::new(visualization::VisualizationServiceBuilder::new(false))],
                })
            } else {
                None
            },
            tick_ms: 1000,
            data: DataPlaneCfg {
                worker_id: 0,
                services: vec![Arc::new(visualization::VisualizationServiceBuilder::new(false))],
                history: Arc::new(DataWorkerHistory::default()),
            },
        };

        let mut queue = VecDeque::from([Output::Bus(BusControl::Channel(Owner::Sdn, BusChannelControl::Subscribe(Channel::Worker(index))))]);

        if sdn_config.controller.is_some() {
            queue.push_back(Output::Bus(BusControl::Channel(Owner::Sdn, BusChannelControl::Subscribe(Channel::Controller))));
        }

        let worker = MediaServerWorker::new(cfg.node.udp_port, sdn_config, cfg.media);
        log::info!("creted worker");
        MediaRuntimeWorker { index, worker, queue }
    }

    fn worker_index(&self) -> u16 {
        self.index
    }

    fn tasks(&self) -> usize {
        self.worker.tasks()
    }

    fn spawn(&mut self, _now: Instant, _cfg: SCfg) {
        panic!("Not supported")
    }

    fn on_tick(&mut self, now: Instant) {
        self.worker.on_tick(now);
    }

    fn on_event(&mut self, now: Instant, event: Input) {
        self.worker.on_event(now, Self::convert_input(event));
    }

    fn pop_output(&mut self, now: Instant) -> Option<Output> {
        if !self.queue.is_empty() {
            return self.queue.pop_front();
        }
        let out = self.worker.pop_output(now)?;
        Some(self.process_out(out))
    }

    fn on_shutdown(&mut self, now: Instant) {
        self.worker.shutdown(now);
    }
}

impl<ES: MediaEdgeSecure> MediaRuntimeWorker<ES> {
    fn process_out(&mut self, out: WorkerOutput) -> Output {
        match out {
            WorkerOutput::ExtRpc(req_id, res) => Output::Ext(true, ExtOut::Rpc(req_id, self.index, res)),
            WorkerOutput::ExtSdn(out) => Output::Ext(false, ExtOut::Sdn(out)),
            WorkerOutput::Bus(event) => match &event {
                SdnWorkerBusEvent::Control(_) => Output::Bus(BusControl::Channel(Owner::Sdn, BusChannelControl::Publish(Channel::Controller, true, event))),
                SdnWorkerBusEvent::Workers(_) => Output::Bus(BusControl::Broadcast(true, event)),
                SdnWorkerBusEvent::Worker(index, _) => Output::Bus(BusControl::Channel(Owner::Sdn, BusChannelControl::Publish(Channel::Worker(*index), true, event))),
            },
            WorkerOutput::Net(owner, out) => Output::Net(owner, out),
            WorkerOutput::Continue => Output::Continue,
        }
    }

    fn convert_input(input: Input) -> WorkerInput {
        match input {
            Input::Bus(event) => match event {
                BusEvent::Broadcast(_from, msg) => WorkerInput::Bus(msg),
                BusEvent::Channel(_owner, _channel, msg) => WorkerInput::Bus(msg),
            },
            Input::Ext(ext) => match ext {
                ExtIn::Rpc(req_id, ext) => WorkerInput::ExtRpc(req_id, ext),
                ExtIn::Sdn(ext) => WorkerInput::ExtSdn(ext),
            },
            Input::Net(owner, event) => WorkerInput::Net(owner, event),
        }
    }
}
