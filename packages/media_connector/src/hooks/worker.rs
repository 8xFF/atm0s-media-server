use std::sync::Arc;

use clap::ValueEnum;
use media_server_multi_tenancy::MultiTenancyStorage;
use media_server_protocol::{multi_tenancy::AppId, protobuf::cluster_connector::HookEvent};
use prost::Message;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum HookBodyType {
    ProtobufJson,
    ProtobufBinary,
}

pub struct HookWorker {
    hook_body_type: HookBodyType,
    app_storage: Arc<MultiTenancyStorage>,
    rx: UnboundedReceiver<(AppId, HookEvent)>,
}

impl HookWorker {
    pub fn new(hook_body_type: HookBodyType, app_storage: Arc<MultiTenancyStorage>) -> (Self, UnboundedSender<(AppId, HookEvent)>) {
        let (tx, rx) = unbounded_channel();
        (Self { hook_body_type, app_storage, rx }, tx)
    }

    pub async fn recv(&mut self) -> Result<(), String> {
        let (app, event) = self.rx.recv().await.ok_or("Internal queue error".to_string())?;
        let hook_url = match self.app_storage.get_app(&app) {
            Some(app) => match app.hook {
                Some(hook) => hook,
                None => return Ok(()),
            },
            None => return Ok(()),
        };
        let client = reqwest::Client::new();
        if let Err(e) = match self.hook_body_type {
            HookBodyType::ProtobufJson => client.post(&hook_url).json(&event).send().await,
            HookBodyType::ProtobufBinary => {
                let mut buf = vec![];
                match event.encode(&mut buf) {
                    Ok(_) => client.post(&hook_url).body(buf).send().await,
                    Err(e) => {
                        log::error!("[HookWorker] encode event to binary error {e:?}");
                        return Ok(());
                    }
                }
            }
        } {
            // TODO: put to retry queue here
            log::error!("[HookWorker] send event error {e:?}");
        } else {
            log::info!("[HookWorker] sent event");
        }
        Ok(())
    }
}
