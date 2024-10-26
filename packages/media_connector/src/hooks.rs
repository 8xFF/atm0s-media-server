use std::sync::Arc;

use media_server_multi_tenancy::MultiTenancyStorage;
use media_server_protocol::{multi_tenancy::AppId, protobuf::cluster_connector::HookEvent};
use tokio::sync::mpsc::UnboundedSender;
use worker::HookWorker;

pub use worker::HookBodyType;

mod worker;

pub struct ConnectorHookSender {
    workers_tx: Vec<UnboundedSender<(AppId, HookEvent)>>,
}

impl ConnectorHookSender {
    pub fn new(workers: usize, hook_body_type: HookBodyType, app_storage: Arc<MultiTenancyStorage>) -> Self {
        let mut workers_tx = vec![];
        for id in 0..workers {
            let (mut worker, tx) = HookWorker::new(hook_body_type, app_storage.clone());
            workers_tx.push(tx);

            tokio::spawn(async move {
                log::info!("[ConnectorHookWorker {id}] started");
                loop {
                    if let Err(e) = worker.recv().await {
                        log::error!("[ConnectorHookWorker {id}] error {e}");
                        break;
                    }
                }
                log::info!("[ConnectorHookWorker {id}] ended");
            });
        }

        Self { workers_tx }
    }

    pub fn on_event(&self, app: AppId, event: HookEvent) {
        let index = event.ts as usize % self.workers_tx.len();
        // TODO handle case worker crash
        self.workers_tx[index].send((app, event)).expect("Should send to worker");
    }
}
