use media_server_protocol::protobuf::cluster_connector::HookEvent;
use tokio::sync::mpsc::UnboundedSender;
use worker::HookWorker;

pub use worker::HookBodyType;

mod worker;

pub struct ConnectorHookSender {
    workers_tx: Vec<UnboundedSender<HookEvent>>,
}

impl ConnectorHookSender {
    pub fn new(workers: usize, hook_body_type: HookBodyType, hook_url: &str) -> Self {
        let mut workers_tx = vec![];
        for id in 0..workers {
            let (mut worker, tx) = HookWorker::new(hook_body_type, hook_url);
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

    pub fn on_event(&self, event: HookEvent) {
        let index = event.ts as usize % self.workers_tx.len();
        // TODO handle case worker crash
        self.workers_tx[index].send(event).expect("Should send to worker");
    }
}
