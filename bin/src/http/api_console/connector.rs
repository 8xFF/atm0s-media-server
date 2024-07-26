use crate::http::Pagination;

use super::{super::Response, ConsoleApisCtx, ConsoleAuthorization};
use media_server_protocol::{
    connector::CONNECTOR_RPC_PORT,
    protobuf::cluster_connector::{GetEventParams, GetParams, GetPeerParams},
    rpc::node_vnet_addr,
};
use poem::web::Data;
use poem_openapi::{
    param::{Path, Query},
    payload::Json,
    OpenApi,
};

#[derive(poem_openapi::Object)]
pub struct RoomInfo {
    pub id: i32,
    pub room: String,
    pub created_at: u64,
    pub peers: usize,
}

#[derive(poem_openapi::Object)]
pub struct PeerSession {
    pub id: i32,
    /// u64 cause wrong parse in js, so we convert it to string
    pub session: String,
    pub peer_id: i32,
    pub peer: String,
    pub created_at: u64,
    pub joined_at: u64,
    pub leaved_at: Option<u64>,
}

#[derive(poem_openapi::Object)]
pub struct PeerInfo {
    pub id: i32,
    pub room_id: i32,
    pub room: String,
    pub peer: String,
    pub created_at: u64,
    pub sessions: Vec<PeerSession>,
}

#[derive(poem_openapi::Object)]
pub struct SessionInfo {
    /// u64 cause wrong parse in js, so we convert it to string
    pub id: String,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub sdk: Option<String>,
    pub created_at: u64,
    pub sessions: Vec<PeerSession>,
}

#[derive(poem_openapi::Object)]
pub struct EventInfo {
    pub id: i32,
    /// u64 cause wrong parse in js, so we convert it to string
    pub session: String,
    pub node: u32,
    pub node_ts: u64,
    pub created_at: u64,
    pub event: String,
    pub meta: Option<String>,
}

pub struct Apis;

#[OpenApi]
impl Apis {
    /// get rooms
    #[oai(path = "/:node/log/rooms", method = "get")]
    async fn rooms(&self, _auth: ConsoleAuthorization, Data(ctx): Data<&ConsoleApisCtx>, Path(node): Path<u32>, Query(page): Query<u32>, Query(limit): Query<u32>) -> Json<Response<Vec<RoomInfo>>> {
        match ctx.connector.rooms(node_vnet_addr(node, CONNECTOR_RPC_PORT), GetParams { page, limit }).await {
            Some(res) => Json(Response {
                status: true,
                data: Some(
                    res.rooms
                        .into_iter()
                        .map(|e| RoomInfo {
                            id: e.id,
                            room: e.room,
                            created_at: e.created_at,
                            peers: e.peers as usize,
                        })
                        .collect::<Vec<_>>(),
                ),
                pagination: res.pagination.map(|p| Pagination {
                    total: p.total as usize,
                    current: p.current as usize,
                }),
                ..Default::default()
            }),
            None => Json(Response {
                status: false,
                error: Some("CLUSTER_ERROR".to_string()),
                ..Default::default()
            }),
        }
    }

    /// get peers
    #[oai(path = "/:node/log/peers", method = "get")]
    async fn peers(
        &self,
        _auth: ConsoleAuthorization,
        Data(ctx): Data<&ConsoleApisCtx>,
        Path(node): Path<u32>,
        Query(room): Query<Option<i32>>,
        Query(page): Query<u32>,
        Query(limit): Query<u32>,
    ) -> Json<Response<Vec<PeerInfo>>> {
        match ctx.connector.peers(node_vnet_addr(node, CONNECTOR_RPC_PORT), GetPeerParams { room, page, limit }).await {
            Some(res) => Json(Response {
                status: true,
                data: Some(
                    res.peers
                        .into_iter()
                        .map(|p| PeerInfo {
                            id: p.id,
                            room_id: p.room_id,
                            room: p.room,
                            peer: p.peer,
                            created_at: p.created_at,
                            sessions: p
                                .sessions
                                .into_iter()
                                .map(|s| PeerSession {
                                    id: s.id,
                                    session: s.session.to_string(),
                                    peer_id: s.peer_id,
                                    peer: s.peer,
                                    created_at: s.created_at,
                                    joined_at: s.joined_at,
                                    leaved_at: s.leaved_at,
                                })
                                .collect::<Vec<_>>(),
                        })
                        .collect::<Vec<_>>(),
                ),
                pagination: res.pagination.map(|p| Pagination {
                    total: p.total as usize,
                    current: p.current as usize,
                }),
                ..Default::default()
            }),
            None => Json(Response {
                status: false,
                error: Some("CLUSTER_ERROR".to_string()),
                ..Default::default()
            }),
        }
    }

    /// get peers
    #[oai(path = "/:node/log/sessions", method = "get")]
    async fn sessions(
        &self,
        _auth: ConsoleAuthorization,
        Data(ctx): Data<&ConsoleApisCtx>,
        Path(node): Path<u32>,
        Query(page): Query<u32>,
        Query(limit): Query<u32>,
    ) -> Json<Response<Vec<SessionInfo>>> {
        match ctx.connector.sessions(node_vnet_addr(node, CONNECTOR_RPC_PORT), GetParams { page, limit }).await {
            Some(res) => Json(Response {
                status: true,
                data: Some(
                    res.sessions
                        .into_iter()
                        .map(|p| SessionInfo {
                            id: p.id.to_string(),
                            ip: p.ip,
                            user_agent: p.user_agent,
                            sdk: p.sdk,
                            created_at: p.created_at,
                            sessions: p
                                .peers
                                .into_iter()
                                .map(|s| PeerSession {
                                    id: s.id,
                                    session: s.session.to_string(),
                                    peer_id: s.peer_id,
                                    peer: s.peer,
                                    created_at: s.created_at,
                                    joined_at: s.joined_at,
                                    leaved_at: s.leaved_at,
                                })
                                .collect::<Vec<_>>(),
                        })
                        .collect::<Vec<_>>(),
                ),
                pagination: res.pagination.map(|p| Pagination {
                    total: p.total as usize,
                    current: p.current as usize,
                }),
                ..Default::default()
            }),
            None => Json(Response {
                status: false,
                error: Some("CLUSTER_ERROR".to_string()),
                ..Default::default()
            }),
        }
    }

    /// get events
    #[allow(clippy::too_many_arguments)]
    #[oai(path = "/:node/log/events", method = "get")]
    async fn events(
        &self,
        _auth: ConsoleAuthorization,
        Data(ctx): Data<&ConsoleApisCtx>,
        Path(node): Path<u32>,
        Query(session): Query<Option<u64>>,
        Query(start_ts): Query<Option<u64>>,
        Query(end_ts): Query<Option<u64>>,
        Query(page): Query<u32>,
        Query(limit): Query<u32>,
    ) -> Json<Response<Vec<EventInfo>>> {
        match ctx
            .connector
            .events(
                node_vnet_addr(node, CONNECTOR_RPC_PORT),
                GetEventParams {
                    session,
                    start_ts,
                    end_ts,
                    page,
                    limit,
                },
            )
            .await
        {
            Some(res) => Json(Response {
                status: true,
                data: Some(
                    res.events
                        .into_iter()
                        .map(|e| EventInfo {
                            id: e.id,
                            session: e.session.to_string(),
                            node: e.node,
                            node_ts: e.node_ts,
                            created_at: e.created_at,
                            event: e.event,
                            meta: e.meta,
                        })
                        .collect::<Vec<_>>(),
                ),
                pagination: res.pagination.map(|p| Pagination {
                    total: p.total as usize,
                    current: p.current as usize,
                }),
                ..Default::default()
            }),
            None => Json(Response {
                status: false,
                error: Some("CLUSTER_ERROR".to_string()),
                ..Default::default()
            }),
        }
    }
}
