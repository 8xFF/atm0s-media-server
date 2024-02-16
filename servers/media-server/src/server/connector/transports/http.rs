use std::{io, marker::PhantomData};

use async_trait::async_trait;
use prost::Message;
use serde::Serialize;

use super::{ConnectorTransporter, Format};

pub struct HttpTransporter<M: Message + Serialize> {
    client: reqwest::Client,
    url: String,
    format: Format,
    _tmp: PhantomData<M>,
}

impl<M: Message + Serialize> HttpTransporter<M> {
    pub fn new(url: &str, format: &Format) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.to_string(),
            format: format.clone(),
            _tmp: Default::default(),
        }
    }
}

#[async_trait]
impl<M: Message + Serialize> ConnectorTransporter<M> for HttpTransporter<M> {
    async fn send(&mut self, data: M) -> Result<(), io::Error> {
        match self.format {
            Format::Json => {
                self.client.post(&self.url).json(&data).send().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            }
            Format::Protobuf => {
                self.client
                    .post(&self.url)
                    .body(data.encode_to_vec())
                    .header("Content-Type", "application/octet-stream")
                    .send()
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            }
        };
        log::debug!("Sending data to {}: {:?}", self.url, data);
        Ok(())
    }

    async fn close(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}
