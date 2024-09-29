pub mod agent_service;
mod store;
pub mod store_service;

pub use store::{MultiTenancyStorage, MultiTenancySync};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ServiceKind {
    Webrtc,
    RtpEngine,
}

#[derive(Debug, Clone, Default)]
pub struct NodeMetrics {
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
}

pub const DATA_PORT: u16 = 10001;

pub const STORE_SERVICE_ID: u8 = 101;
pub const STORE_SERVICE_NAME: &str = "gateway_store";

pub const AGENT_SERVICE_ID: u8 = 102;
pub const AGENT_SERVICE_NAME: &str = "gateway_agent";
