use std::{sync::Arc, time::Instant};

use atm0s_sdn::{
    secure::{HandshakeBuilderXDA, StaticKeyAuthorization},
    services::visualization,
    ControllerPlaneCfg, DataPlaneCfg, DataWorkerHistory, SdnExtOut,
};
use media_server_protocol::transport::{RpcReq, RpcRes};
use media_server_runner::{Input as WorkerInput, MediaConfig, MediaServerWorker, Output as WorkerOutput, Owner, SdnConfig, SE};
use rand::rngs::OsRng;
use sans_io_runtime::{WorkerInner, WorkerInnerInput, WorkerInnerOutput};

use crate::NodeConfig;

#[derive(Debug, Clone)]
pub enum ExtIn {
    Rpc(u64, RpcReq<usize>),
}

#[derive(Debug, Clone)]
pub enum ExtOut {
    Rpc(u64, u16, RpcRes<usize>),
    Sdn(SdnExtOut<SE>),
}
type Channel = ();
type Event = ();
pub struct ICfg {
    pub controller: bool,
    pub node: NodeConfig,
    pub media: MediaConfig,
}
type SCfg = ();

type Input<'a> = WorkerInnerInput<'a, Owner, ExtIn, Channel, Event>;
type Output<'a> = WorkerInnerOutput<'a, Owner, ExtOut, Channel, Event, SCfg>;

pub struct MediaRuntimeWorker {
    index: u16,
    worker: MediaServerWorker,
}

impl WorkerInner<Owner, ExtIn, ExtOut, Channel, Event, ICfg, SCfg> for MediaRuntimeWorker {
    fn build(index: u16, cfg: ICfg) -> Self {
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
            tick_ms: 1,
            data: DataPlaneCfg {
                worker_id: 0,
                services: vec![Arc::new(visualization::VisualizationServiceBuilder::new(false))],
                history: Arc::new(DataWorkerHistory::default()),
            },
        };

        MediaRuntimeWorker {
            index,
            worker: MediaServerWorker::new(sdn_config, cfg.media),
        }
    }

    fn worker_index(&self) -> u16 {
        self.index
    }

    fn tasks(&self) -> usize {
        self.worker.tasks()
    }

    fn spawn(&mut self, now: Instant, cfg: SCfg) {
        todo!()
    }

    fn on_tick<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        let out = self.worker.on_tick(now)?;
        Some(self.process_out(out))
    }

    fn on_event<'a>(&mut self, now: Instant, event: Input<'a>) -> Option<Output<'a>> {
        let out = self.worker.on_event(now, Self::convert_input(event))?;
        Some(self.process_out(out))
    }

    fn pop_output<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        let out = self.worker.pop_output(now)?;
        Some(self.process_out(out))
    }

    fn shutdown<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        let out = self.worker.shutdown(now)?;
        Some(self.process_out(out))
    }
}

impl MediaRuntimeWorker {
    fn process_out<'a>(&mut self, out: WorkerOutput<'a>) -> Output<'a> {
        match out {
            WorkerOutput::ExtRpc(req_id, res) => Output::Ext(true, ExtOut::Rpc(req_id, self.index, res)),
            WorkerOutput::ExtSdn(out) => Output::Ext(false, ExtOut::Sdn(out)),
            WorkerOutput::Bus(event) => {
                todo!()
            }
            WorkerOutput::Net(owner, out) => Output::Net(owner, out),
            WorkerOutput::Continue => Output::Continue,
        }
    }

    fn convert_input<'a>(input: Input<'a>) -> WorkerInput<'a> {
        match input {
            Input::Bus(event) => {
                todo!()
            }
            Input::Ext(ext) => match ext {
                ExtIn::Rpc(req_id, ext) => WorkerInput::ExtRpc(req_id, ext),
            },
            Input::Net(owner, event) => WorkerInput::Net(owner, event),
        }
    }
}
