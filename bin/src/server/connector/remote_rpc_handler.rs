use std::sync::Arc;

use media_server_connector::Querier;
use media_server_protocol::protobuf::cluster_connector::{
    get_events::EventInfo, get_peers::PeerInfo, get_rooms::RoomInfo, get_sessions::SessionInfo, GetEventParams, GetEvents, GetParams, GetPeerParams, GetPeers, GetRooms, GetSessions,
    MediaConnectorServiceHandler, PeerSession,
};
use media_server_protocol::protobuf::shared::Pagination;

#[derive(Clone)]
pub struct Ctx {
    pub storage: Arc<dyn Querier>, //TODO make it generic
}

#[derive(Default)]
pub struct ConnectorRemoteRpcHandlerImpl {}

impl MediaConnectorServiceHandler<Ctx> for ConnectorRemoteRpcHandlerImpl {
    async fn rooms(&self, ctx: &Ctx, req: GetParams) -> Option<GetRooms> {
        log::info!("[ConnectorRemoteRpcHandler] on get rooms {req:?}");
        let response = match ctx.storage.rooms(req.page as usize, req.limit as usize).await {
            Ok(res) => res,
            Err(err) => {
                log::error!("[ConnectorRemoteRpcHandler] on get rooms error {err}");
                return None;
            }
        };
        log::info!("[ConnectorRemoteRpcHandler] on got {} rooms", response.data.len());

        let rooms = response
            .data
            .into_iter()
            .map(|e| RoomInfo {
                id: e.id,
                room: e.room,
                created_at: e.created_at,
                peers: e.peers as u32,
            })
            .collect::<Vec<_>>();

        Some(GetRooms {
            rooms,
            pagination: Some(Pagination {
                total: response.total as u32,
                current: response.current as u32,
            }),
        })
    }

    async fn peers(&self, ctx: &Ctx, req: GetPeerParams) -> Option<GetPeers> {
        log::info!("[ConnectorRemoteRpcHandler] on get peers page {req:?}");
        let response = match ctx.storage.peers(req.room, req.page as usize, req.limit as usize).await {
            Ok(res) => res,
            Err(err) => {
                log::error!("[ConnectorRemoteRpcHandler] on get peers error {err}");
                return None;
            }
        };
        log::info!("[ConnectorRemoteRpcHandler] on got {} peers", response.data.len());

        let peers = response
            .data
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
                    .map(|p| PeerSession {
                        id: p.id,
                        peer_id: p.peer_id,
                        peer: p.peer,
                        session: p.session,
                        created_at: p.created_at,
                        joined_at: p.joined_at,
                        leaved_at: p.leaved_at,
                    })
                    .collect::<Vec<_>>(),
            })
            .collect::<Vec<_>>();

        Some(GetPeers {
            peers,
            pagination: Some(Pagination {
                total: response.total as u32,
                current: response.current as u32,
            }),
        })
    }

    async fn sessions(&self, ctx: &Ctx, req: GetParams) -> Option<GetSessions> {
        log::info!("[ConnectorRemoteRpcHandler] on get sessions page {req:?}");
        let response = match ctx.storage.sessions(req.page as usize, req.limit as usize).await {
            Ok(res) => res,
            Err(err) => {
                log::error!("[ConnectorRemoteRpcHandler] on get sessions error {err}");
                return None;
            }
        };
        log::info!("[ConnectorRemoteRpcHandler] on got {} sessions", response.data.len());

        let sessions = response
            .data
            .into_iter()
            .map(|e| SessionInfo {
                id: e.id,
                ip: e.ip,
                sdk: e.sdk,
                user_agent: e.user_agent,
                created_at: e.created_at,
                peers: e
                    .peers
                    .into_iter()
                    .map(|p| PeerSession {
                        id: p.id,
                        peer_id: p.peer_id,
                        peer: p.peer,
                        session: p.session,
                        created_at: p.created_at,
                        joined_at: p.joined_at,
                        leaved_at: p.leaved_at,
                    })
                    .collect::<Vec<_>>(),
            })
            .collect::<Vec<_>>();
        Some(GetSessions {
            sessions,
            pagination: Some(Pagination {
                total: response.total as u32,
                current: response.current as u32,
            }),
        })
    }

    async fn events(&self, ctx: &Ctx, req: GetEventParams) -> Option<GetEvents> {
        log::info!("[ConnectorRemoteRpcHandler] on get events page {req:?}");
        let response = match ctx.storage.events(req.session, req.start_ts, req.end_ts, req.page as usize, req.limit as usize).await {
            Ok(res) => res,
            Err(err) => {
                log::error!("[ConnectorRemoteRpcHandler] on get events error {err}");
                return None;
            }
        };
        log::info!("[ConnectorRemoteRpcHandler] on got {} events", response.data.len());

        let events = response
            .data
            .into_iter()
            .map(|e| EventInfo {
                id: e.id,
                node: e.node,
                node_ts: e.node_ts,
                session: e.session,
                created_at: e.created_at,
                event: e.event,
                meta: e.meta.map(|m| m.to_string()),
            })
            .collect::<Vec<_>>();
        Some(GetEvents {
            events,
            pagination: Some(Pagination {
                total: response.total as u32,
                current: response.current as u32,
            }),
        })
    }
}
