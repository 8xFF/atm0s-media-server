use std::io;

use async_trait::async_trait;
use prost::Message;

pub mod nats;

#[derive(Debug, PartialEq, Eq)]
pub enum ParseURIError {
    InvalidURI,
}

#[async_trait]
pub trait ConnectorTransporter<M: Message + TryFrom<Vec<u8>>>: Send + Sync {
    async fn close(&mut self) -> Result<(), io::Error>;
    async fn send(&mut self, data: M) -> Result<(), io::Error>;
}

pub fn parse_uri(uri: &str) -> Result<(String, String), ParseURIError> {
    let mut parts = uri.splitn(2, "://");
    let transport = parts.next().ok_or(ParseURIError::InvalidURI)?;
    let uri = parts.next().ok_or(ParseURIError::InvalidURI)?;
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
