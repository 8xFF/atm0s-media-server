use clap::ValueEnum;
use media_server_protocol::protobuf::cluster_connector::HookEvent;
use prost::Message;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum HookBodyType {
    ProtobufJson,
    ProtobufBinary,
}

pub struct HookWorker {
    hook_body_type: HookBodyType,
    hook_url: String,
    rx: UnboundedReceiver<HookEvent>,
}

impl HookWorker {
    pub fn new(hook_body_type: HookBodyType, hook_url: &str) -> (Self, UnboundedSender<HookEvent>) {
        let (tx, rx) = unbounded_channel();
        (
            Self {
                hook_body_type,
                hook_url: hook_url.to_owned(),
                rx,
            },
            tx,
        )
    }

    pub async fn recv(&mut self) -> Result<(), String> {
        let event = self.rx.recv().await.ok_or("Internal queue error".to_string())?;
        let client = reqwest::Client::new();
        if let Err(e) = match self.hook_body_type {
            HookBodyType::ProtobufJson => client.post(&self.hook_url).json(&event).send().await,
            HookBodyType::ProtobufBinary => {
                let mut buf = vec![];
                match event.encode(&mut buf) {
                    Ok(_) => client.post(&self.hook_url).body(buf).send().await,
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
