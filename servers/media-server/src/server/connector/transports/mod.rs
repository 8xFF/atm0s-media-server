use async_trait::async_trait;

pub mod nats;

#[async_trait]
pub trait ConnectorTransporter: Send + Sync {
    async fn send(&self, data: &[u8]) -> Result<(), String>;
    async fn close(&mut self) -> Result<(), String>;
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
