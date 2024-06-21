pub mod agent_service;
pub mod handler_service;
mod msg_queue;

pub const DATA_PORT: u16 = 10002;

pub const AGENT_SERVICE_ID: u8 = 103;
pub const AGENT_SERVICE_NAME: &str = "connector-agent";
pub const HANDLER_SERVICE_ID: u8 = 104;
pub const HANDLER_SERVICE_NAME: &str = "connector-handler";
