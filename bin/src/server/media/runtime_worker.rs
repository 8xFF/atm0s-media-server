use std::time::Instant;

use media_server_protocol::transport::{RpcReq, RpcRes};
use sans_io_runtime::{WorkerInner, WorkerInnerInput, WorkerInnerOutput};

type Owner = ();

#[derive(Debug, Clone)]
pub enum ExtIn {
    Rpc(u64, RpcReq),
}

#[derive(Debug, Clone)]
pub enum ExtOut {
    Rpc(u64, RpcRes),
}
type Channel = ();
type Event = ();
pub type ICfg = ();
type SCfg = ();

type Input<'a> = WorkerInnerInput<'a, Owner, ExtIn, Channel, Event>;
type Output<'a> = WorkerInnerOutput<'a, Owner, ExtOut, Channel, Event, SCfg>;

pub struct MediaRuntimeWorker {
    index: u16,
}

impl WorkerInner<Owner, ExtIn, ExtOut, Channel, Event, ICfg, SCfg> for MediaRuntimeWorker {
    fn build(worker: u16, cfg: ICfg) -> Self {
        MediaRuntimeWorker { index: worker }
    }

    fn worker_index(&self) -> u16 {
        self.index
    }

    fn tasks(&self) -> usize {
        0
    }

    fn spawn(&mut self, now: Instant, cfg: SCfg) {
        todo!()
    }

    fn on_tick<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        todo!()
    }

    fn on_event<'a>(&mut self, now: Instant, event: Input<'a>) -> Option<Output<'a>> {
        todo!()
    }

    fn pop_output<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        todo!()
    }

    fn shutdown<'a>(&mut self, now: Instant) -> Option<Output<'a>> {
        todo!()
    }
}
