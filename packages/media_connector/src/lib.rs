use atm0s_sdn::NodeId;
use media_server_protocol::protobuf::cluster_connector::{connector_request, connector_response};
use serde_json::Value;

pub mod agent_service;
pub mod handler_service;
mod msg_queue;
pub mod sql_storage;

pub const DATA_PORT: u16 = 10002;

pub const AGENT_SERVICE_ID: u8 = 103;
pub const AGENT_SERVICE_NAME: &str = "connector-agent";
pub const HANDLER_SERVICE_ID: u8 = 104;
pub const HANDLER_SERVICE_NAME: &str = "connector-handler";

#[derive(Debug)]
pub struct RoomInfo {
    pub id: i32,
    pub room: String,
    pub created_at: u64,
    pub peers: usize,
}

#[derive(Debug)]
pub struct PeerSession {
    pub id: i32,
    pub peer_id: i32,
    pub peer: String,
    pub session: u64,
    pub created_at: u64,
    pub joined_at: u64,
    pub leaved_at: Option<u64>,
}

#[derive(Debug)]
pub struct PeerInfo {
    pub id: i32,
    pub room_id: i32,
    pub room: String,
    pub peer: String,
    pub created_at: u64,
    pub sessions: Vec<PeerSession>,
}

#[derive(Debug)]
pub struct SessionInfo {
    pub id: u64,
    pub created_at: u64,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub sdk: Option<String>,
    pub peers: Vec<PeerSession>,
}

#[derive(Debug)]
pub struct EventInfo {
    pub id: i32,
    pub node: u32,
    pub session: u64,
    pub node_ts: u64,
    pub created_at: u64,
    pub event: String,
    pub meta: Option<Value>,
}

pub trait Storage {
    fn on_event(&self, from: NodeId, ts: u64, req: connector_request::Request) -> impl std::future::Future<Output = Option<connector_response::Response>> + Send;
}

pub trait Querier {
    fn rooms(&self, page: usize, count: usize) -> impl std::future::Future<Output = Option<Vec<RoomInfo>>> + Send;
    fn peers(&self, room: Option<i32>, page: usize, count: usize) -> impl std::future::Future<Output = Option<Vec<PeerInfo>>> + Send;
    fn sessions(&self, page: usize, count: usize) -> impl std::future::Future<Output = Option<Vec<SessionInfo>>> + Send;
    fn events(&self, session: Option<u64>, from: Option<u64>, to: Option<u64>, page: usize, count: usize) -> impl std::future::Future<Output = Option<Vec<EventInfo>>> + Send;
}
