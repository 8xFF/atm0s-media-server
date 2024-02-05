use std::{io, marker::PhantomData, time::Duration};

use async_trait::async_trait;
use prost::Message;

use super::ConnectorTransporter;

pub struct NatsTransporter<M: Message + Clone + TryFrom<Vec<u8>>> {
    conn: nats::asynk::Connection,
    subject: String,
    _tmp: PhantomData<M>,
}

impl<M: Message + Clone + TryFrom<Vec<u8>>> NatsTransporter<M> {
    pub async fn new(uri: String, subject: String) -> Result<Self, io::Error> {
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
            subject,
            _tmp: Default::default(),
        })
    }
}

#[async_trait]
impl<M: Message + Clone + TryFrom<Vec<u8>>> ConnectorTransporter<M> for NatsTransporter<M> {
    async fn send(&mut self, data: M) -> Result<(), io::Error> {
        let data = data.encode_to_vec();
        self.conn.publish(&self.subject, data).await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), io::Error> {
        self.conn.close().await?;
        Ok(())
    }
}
