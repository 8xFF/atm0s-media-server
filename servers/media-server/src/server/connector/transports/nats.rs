use super::Transporter;

pub struct NatsTransporter {
    pub conn: nats::Connection,
    pub subject: String,
    pub sub: Option<nats::Subscription>,
}

impl NatsTransporter {
    pub fn new(uri: String, subject: String) -> Result<Self, String> {
        let res = nats::connect(&uri);

        let conn = match res {
            Ok(conn) => conn,
            Err(e) => {
                return Err(e.to_string());
            }
        };

        Ok(Self { conn, subject, sub: None })
    }
}

impl Transporter for NatsTransporter {
    fn send(&self, data: &[u8]) -> Result<(), String> {
        self.conn.publish(&self.subject, data).map_err(|e| e.to_string())?;
        return Ok(());
    }

    fn close(&mut self) {
        if let Some(sub) = self.sub.take() {
            sub.unsubscribe().unwrap();
        }
    }
}
