use std::marker::PhantomData;

use async_trait::async_trait;
use prost::Message;

use super::ConnectorTransporter;

pub struct NatsTransporter<M: Message> {
    pub conn: nats::asynk::Connection,
    pub subject: String,
    pub sub: Option<nats::asynk::Subscription>,
    _tmp: PhantomData<M>,
}

impl<M: Message> NatsTransporter<M> {
    pub async fn new(uri: String, subject: String) -> Result<Self, String> {
        let res = nats::asynk::connect(&uri).await;

        let conn = match res {
            Ok(conn) => conn,
            Err(e) => {
                return Err(e.to_string());
            }
        };

        Ok(Self {
            conn,
            subject,
            sub: None,
            _tmp: Default::default(),
        })
    }
}

#[async_trait]
impl<M: Message> ConnectorTransporter<M> for NatsTransporter<M> {
    async fn send(&self, data: &M) -> Result<(), String> {
        let data: Vec<u8> = data.encode_to_vec();
        self.conn.publish(&self.subject, data).await.map_err(|e| e.to_string())?;
        return Ok(());
    }

    async fn close(&mut self) -> Result<(), String> {
        if let Some(sub) = self.sub.take() {
            let _ = sub.unsubscribe().await.map_err(|e: std::io::Error| e.to_string())?;
        }
        Ok(())
    }
}
