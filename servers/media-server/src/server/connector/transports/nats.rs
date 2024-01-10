use std::{collections::VecDeque, marker::PhantomData};

use async_std::channel::Receiver;
use async_trait::async_trait;
use prost::Message;

use super::ConnectorTransporter;

pub struct NatsTransporter<M: Message + Clone> {
    pub conn: Option<nats::asynk::Connection>,
    pub uri: String,
    pub subject: String,
    pub rx: Receiver<M>,
    _memory_logs: VecDeque<M>,
    _tmp: PhantomData<M>,
}

impl<M: Message + Clone> NatsTransporter<M> {
    pub fn new(uri: String, subject: String, rx: Receiver<M>) -> Self {
        Self {
            uri,
            rx,
            conn: None,
            subject,
            _memory_logs: Default::default(),
            _tmp: Default::default(),
        }
    }

    async fn _send(&self, data: &M) -> Result<(), String> {
        let data: Vec<u8> = data.encode_to_vec();
        if let Some(conn) = &self.conn {
            conn.publish(&self.subject, data).await.map_err(|e| e.to_string())?;
        } else {
            return Err("MQ connection not established".to_string());
        }
        Ok(())
    }

    async fn _try_send_memory_logs(&mut self) {
        while let Some(queue_data) = self._memory_logs.get(0) {
            if let Err(e) = self._send(&queue_data).await {
                log::error!("Error sending message: {:?}, saving it into memory for later", e);
                break;
            }
            let _ = self._memory_logs.pop_front();
        }
    }

    async fn connect(&mut self) -> Result<(), String> {
        let conn = nats::asynk::connect(&self.uri).await.map_err(|e| e.to_string())?;
        self.conn = Some(conn);
        Ok(())
    }

    async fn try_send(&mut self, data: &M) -> Result<(), String> {
        let _ = self._memory_logs.push_back(data.clone());

        if self.is_connected() {
            if self._memory_logs.len() > 0 {
                self._try_send_memory_logs().await;
            }
        } else {
            log::error!("MQ connection not established, try to reconnect");
            if let Err(e) = self.connect().await {
                log::error!("Error connecting to MQ: {:?}", e);
            } else {
                self._try_send_memory_logs().await;
            }
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.conn.is_some()
    }
}

#[async_trait]
impl<M: Message + Clone> ConnectorTransporter<M> for NatsTransporter<M> {
    async fn start(&mut self) -> Result<(), String> {
        self.connect().await?;
        Ok(())
    }

    async fn poll(&mut self) -> Result<(), String> {
        while let Ok(data) = self.rx.recv().await {
            let _ = self.try_send(&data).await;
        }
        Ok(())
    }

    async fn close(&mut self) -> Result<(), String> {
        if let Some(conn) = self.conn.take() {
            conn.close().await.map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
