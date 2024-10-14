use std::sync::Arc;

use atm0s_sdn::NodeId;
use hooks::ConnectorHookSender;
use media_server_multi_tenancy::MultiTenancyStorage;
use media_server_protocol::{
    multi_tenancy::AppId,
    protobuf::cluster_connector::{connector_request, connector_response, HookEvent},
};
use media_server_utils::now_ms;
use serde_json::Value;
use sql_storage::{ConnectorSqlQuerier, ConnectorSqlStorage};

pub mod agent_service;
pub mod handler_service;
pub mod hooks;
mod msg_queue;
mod sql_storage;

pub use hooks::HookBodyType;

pub const DATA_PORT: u16 = 10002;

pub const AGENT_SERVICE_ID: u8 = 103;
pub const AGENT_SERVICE_NAME: &str = "connector-agent";
pub const HANDLER_SERVICE_ID: u8 = 104;
pub const HANDLER_SERVICE_NAME: &str = "connector-handler";

#[derive(Debug)]
pub struct PagingResponse<T> {
    pub data: Vec<T>,
    pub total: usize,
    pub current: usize,
}

#[derive(Debug)]
pub struct RoomInfo {
    pub id: i32,
    pub app: String,
    pub room: String,
    pub created_at: u64,
    pub destroyed_at: Option<u64>,
    pub peers: usize,
    pub record: Option<String>,
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
    pub app: String,
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

pub struct ConnectorCfg {
    pub sql_uri: String,
    pub s3_uri: String,
    pub hook_workers: usize,
    pub hook_body_type: HookBodyType,
    pub room_destroy_after_ms: u64,
}

pub trait Storage {
    type Q: Querier;
    fn querier(&mut self) -> Self::Q;
    fn on_tick(&mut self, now_ms: u64) -> impl std::future::Future<Output = ()> + Send;
    fn on_event(&mut self, now_ms: u64, from: NodeId, req_ts: u64, req: connector_request::Request) -> impl std::future::Future<Output = Option<connector_response::Response>> + Send;
    fn pop_hook_event(&mut self) -> Option<(AppId, HookEvent)>;
}

#[async_trait::async_trait]
pub trait Querier {
    async fn rooms(&self, page: usize, count: usize) -> Result<PagingResponse<RoomInfo>, String>;
    async fn peers(&self, room: Option<i32>, page: usize, count: usize) -> Result<PagingResponse<PeerInfo>, String>;
    async fn sessions(&self, page: usize, count: usize) -> Result<PagingResponse<SessionInfo>, String>;
    async fn events(&self, session: Option<u64>, from: Option<u64>, to: Option<u64>, page: usize, count: usize) -> Result<PagingResponse<EventInfo>, String>;
}

pub struct ConnectorStorage {
    sql_storage: ConnectorSqlStorage,
    hook: ConnectorHookSender,
}

impl ConnectorStorage {
    pub async fn new(node: NodeId, app_storage: Arc<MultiTenancyStorage>, cfg: ConnectorCfg) -> Self {
        Self {
            sql_storage: ConnectorSqlStorage::new(node, &cfg).await,
            hook: ConnectorHookSender::new(cfg.hook_workers, cfg.hook_body_type, app_storage),
        }
    }

    pub fn querier(&mut self) -> ConnectorSqlQuerier {
        self.sql_storage.querier()
    }

    pub async fn on_tick(&mut self) {
        self.sql_storage.on_tick(now_ms()).await;
        while let Some((app, event)) = self.sql_storage.pop_hook_event() {
            self.hook.on_event(app, event);
        }
    }

    pub async fn on_event(&mut self, from: NodeId, ts: u64, req: connector_request::Request) -> Option<connector_response::Response> {
        let res = self.sql_storage.on_event(now_ms(), from, ts, req).await?;

        while let Some((app, event)) = self.sql_storage.pop_hook_event() {
            self.hook.on_event(app, event);
        }

        Some(res)
    }
}
