use std::{fmt::Display, io};

use async_trait::async_trait;
use clap::ValueEnum;
use prost::Message;

pub mod nats;
pub mod http;

#[derive(Debug, PartialEq, Eq)]
pub enum ParseURIError {
    InvalidURI,
}

#[derive(Debug, ValueEnum, Clone)]
pub enum Format {
    Json,
    Protobuf,
}

impl Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Json => write!(f, "json"),
            Format::Protobuf => write!(f, "protobuf"),
        }
    }
}

#[async_trait]
pub trait ConnectorTransporter<M: Message>: Send + Sync {
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
