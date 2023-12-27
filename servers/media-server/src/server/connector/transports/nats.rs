use async_trait::async_trait;

use super::ConnectorTransporter;

pub struct NatsTransporter {
    pub conn: nats::asynk::Connection,
    pub subject: String,
    pub sub: Option<nats::asynk::Subscription>,
}

impl NatsTransporter {
    pub async fn new(uri: String, subject: String) -> Result<Self, String> {
        let res = nats::asynk::connect(&uri).await;

        let conn = match res {
            Ok(conn) => conn,
            Err(e) => {
                return Err(e.to_string());
            }
        };

        Ok(Self { conn, subject, sub: None })
    }
}

#[async_trait]
impl ConnectorTransporter for NatsTransporter {
    async fn send(&self, data: &[u8]) -> Result<(), String> {
        self.conn.publish(&self.subject, data).await.map_err(|e| e.to_string())?;
        return Ok(());
    }

    async fn close(&mut self) -> Result<(), String> {
        if let Some(sub) = self.sub.take() {
            let res = sub.unsubscribe().await.map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
