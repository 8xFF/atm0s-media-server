use std::{io, marker::PhantomData, time::Duration};

use async_trait::async_trait;
use prost::Message;
use serde::Serialize;

use super::{ConnectorTransporter, Format};

pub struct NatsTransporter<M: Message + Clone + TryFrom<Vec<u8>> + Serialize> {
    conn: nats::asynk::Connection,
    subject: String,
    format: Format,
    _tmp: PhantomData<M>,
}

impl<M: Message + Clone + TryFrom<Vec<u8>> + Serialize> NatsTransporter<M> {
    pub async fn new(uri: &str, subject: &str, format: &Format) -> Result<Self, io::Error> {
        log::info!("Connecting to NATS server: {}", uri);
        Ok(Self {
            conn: nats::asynk::Options::new()
                .retry_on_failed_connect()
                .max_reconnects(999999999) //big value ensure nats will auto reconnect forever in theory
                .reconnect_delay_callback(|c| Duration::from_millis(std::cmp::min((c * 100) as u64, 10000))) //max wait 10s
                .disconnect_callback(|| log::warn!("connection has been lost"))
                .reconnect_callback(|| log::warn!("connection has been reestablished"))
                .close_callback(|| panic!("connection has been closed")) //this should not happen
                .connect(uri)
                .await?,
            subject: subject.to_string(),
            format: format.clone(),
            _tmp: Default::default(),
        })
    }
}

#[async_trait]
impl<M: Message + Clone + TryFrom<Vec<u8>> + Serialize> ConnectorTransporter<M> for NatsTransporter<M> {
    async fn send(&mut self, data: M) -> Result<(), io::Error> {
        let data: Vec<u8> = match self.format {
            Format::Json => match serde_json::to_string(&data) {
                Ok(data) => data.as_bytes().to_vec(),
                Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
            },
            Format::Protobuf => data.encode_to_vec(),
        };
        self.conn.publish(&self.subject, data).await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), io::Error> {
        self.conn.close().await?;
        Ok(())
    }
}
