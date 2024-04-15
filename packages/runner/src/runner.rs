use atm0s_sdn::SdnWorkerCfg;
use sans_io_runtime::{WorkerInner, WorkerInnerInput, WorkerInnerOutput};
use std::time::Instant;

use crate::worker::{MediaServerWorker, SC, SE, TC, TW};

type Input<'a> = WorkerInnerInput<'a, Owner, ExtIn, Channel, Event>;
type Output<'a> = WorkerInnerOutput<'a, Owner, ExtOut, Channel, Event, SCfg>;

type Owner = ();

//for runner
type Channel = ();
type Event = ();
pub enum ExtIn {}
pub enum ExtOut {}

struct ICfg {
    sdn: SdnWorkerCfg<SC, SE, TC, TW>,
}
type SCfg = ();

pub struct MediaServerRunner {
    worker: u16,
    media_worker: MediaServerWorker,
}

impl WorkerInner<Owner, ExtIn, ExtOut, Channel, Event, ICfg, SCfg> for MediaServerRunner {
    fn build(worker: u16, cfg: ICfg) -> Self {
        Self {
            worker,
            media_worker: MediaServerWorker::new(cfg.sdn),
        }
    }

    fn worker_index(&self) -> u16 {
        self.worker
    }

    fn tasks(&self) -> usize {
        todo!()
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
