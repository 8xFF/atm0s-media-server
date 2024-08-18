use std::io::Error;

use media_server_connector::{hook_producer::HookPublisher, hooks::events::HookEvent};

pub struct HttpHookPublisher {
    uri: String,
    client: reqwest::Client,
}

impl HttpHookPublisher {
    pub fn new(uri: String) -> Self {
        log::info!("[HttpHookPublisher] new uri: {}", uri);
        Self { uri, client: reqwest::Client::new() }
    }
}

#[async_trait::async_trait]
impl HookPublisher for HttpHookPublisher {
    async fn publish(&self, event: HookEvent) -> Option<Error> {
        let res = self.client.post(self.uri.clone()).json(&event).send().await;
        match res {
            Ok(res) => {
                log::debug!("[HttpHookPublisher] publish response {:?}", res);
                if res.status().is_success() {
                    None
                } else {
                    Some(Error::new(std::io::ErrorKind::Other, format!("request error with status: {}", res.status())))
                }
            }
            Err(e) => {
                log::error!("[HttpHookPublisher] publish error {:?}", e);
                Some(Error::new(std::io::ErrorKind::Other, e.to_string()))
            }
        }
    }
}
