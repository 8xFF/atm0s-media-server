use atm0s_sdn::NodeId;
use media_server_protocol::protobuf::cluster_connector::connector_request;
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

pub struct RoomInfo {
    pub id: i32,
    pub room: String,
    pub created_at: u64,
    pub peers: usize,
}

pub struct PeerInfo {
    pub id: i32,
    pub room: i32,
    pub peer: String,
    pub created_at: u64,
    pub sessions: usize,
}

pub struct SessionInfo {
    pub id: u64,
    pub created_at: u64,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub sdk: Option<String>,
    pub events: usize,
}

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
    fn on_event(&self, from: NodeId, ts: u64, req_id: u64, event: connector_request::Event) -> impl std::future::Future<Output = Option<()>> + Send;
}

pub trait Querier {
    fn rooms(&self, page: usize, count: usize) -> impl std::future::Future<Output = Option<Vec<RoomInfo>>> + Send;
    fn room(&self, room: i32) -> impl std::future::Future<Output = Option<RoomInfo>> + Send;
    fn peers(&self, room: Option<i32>, page: usize, count: usize) -> impl std::future::Future<Output = Option<Vec<PeerInfo>>> + Send;
    fn peer(&self, peer: i32) -> impl std::future::Future<Output = Option<PeerInfo>> + Send;
    fn sessions(&self, peer: Option<i32>, page: usize, count: usize) -> impl std::future::Future<Output = Option<Vec<SessionInfo>>> + Send;
    fn session(&self, session: u64) -> impl std::future::Future<Output = Option<SessionInfo>> + Send;
    fn events(&self, session: Option<i32>, page: usize, count: usize) -> impl std::future::Future<Output = Option<Vec<EventInfo>>> + Send;
}
