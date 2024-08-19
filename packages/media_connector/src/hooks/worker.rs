use std::{collections::VecDeque, sync::Arc};

use super::{storage::HookJobData, HookPublisher};

pub struct HookWorker {
    queues: VecDeque<HookJobData>,
    publisher: Option<Arc<dyn HookPublisher>>,
}

impl HookWorker {
    pub fn new(publisher: Option<Arc<dyn HookPublisher>>) -> Self {
        Self { queues: VecDeque::new(), publisher }
    }

    pub fn push(&mut self, data: HookJobData) {
        self.queues.push_back(data);
    }

    pub async fn on_tick(&mut self) {
        while let Some(job) = self.queues.pop_front() {
            if let Some(publisher) = &self.publisher {
                let err = publisher.publish(job.payload.clone()).await;
                if err.is_some() {
                    log::error!("[HookWorker] Failed to publish hook event: {:?}", err);
                    continue;
                }
            }
            job.ack();
        }
    }
}
