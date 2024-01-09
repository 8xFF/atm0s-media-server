use async_trait::async_trait;
use prost::Message;

pub mod nats;

#[async_trait]
pub trait ConnectorTransporter<M: Message>: Send + Sync {
    async fn send(&self, data: &M) -> Result<(), String>;
    async fn close(&mut self) -> Result<(), String>;
    async fn connect(&mut self) -> Result<(), String>;
    async fn try_send(&mut self, data: &M) -> Result<(), String>;
    fn is_connected(&self) -> bool;
}

pub fn parse_uri(uri: &str) -> Result<(String, String), String> {
    let mut parts = uri.splitn(2, "://");
    let transport = parts.next().ok_or("Invalid URI")?;
    let uri = parts.next().ok_or("Invalid URI")?;
    Ok((transport.to_string(), uri.to_string()))
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse_uri() {
        let uri = "nats://localhost:4222";

        let parsed = super::parse_uri(&uri);

        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), ("nats".to_string(), "localhost:4222".to_string()));
    }
}
