pub mod agent_service;
mod store;
pub mod store_service;

#[derive(Debug, Clone)]
pub enum ServiceKind {
    Webrtc,
}

pub const DATA_PORT: u16 = 10001;

pub const STORE_SERVICE_ID: u8 = 101;
pub const STORE_SERVICE_NAME: &str = "gateway_store";

pub const AGENT_SERVICE_ID: u8 = 102;
pub const AGENT_SERVICE_NAME: &str = "gateway_agent";
