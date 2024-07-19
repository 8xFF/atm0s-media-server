use std::sync::Arc;

use media_server_connector::{sql_storage, Querier};
use media_server_protocol::protobuf::cluster_connector::{
    get_events::EventInfo, get_peers::PeerInfo, get_rooms::RoomInfo, get_sessions::SessionInfo, GetEventParams, GetEvents, GetParams, GetPeerParams, GetPeers, GetRooms, GetSessions,
    MediaConnectorServiceHandler, PeerSession,
};

#[derive(Clone)]
pub struct Ctx {
    pub storage: Arc<sql_storage::ConnectorStorage>, //TODO make it generic
}

#[derive(Default)]
pub struct ConnectorRemoteRpcHandlerImpl {}

impl MediaConnectorServiceHandler<Ctx> for ConnectorRemoteRpcHandlerImpl {
    async fn rooms(&self, ctx: &Ctx, req: GetParams) -> Option<GetRooms> {
        let rooms = ctx
            .storage
            .rooms(req.page as usize, req.limit as usize)
            .await?
            .into_iter()
            .map(|e| RoomInfo {
                id: e.id,
                room: e.room,
                created_at: e.created_at,
                peers: e.peers as u32,
            })
            .collect::<Vec<_>>();
        Some(GetRooms { rooms })
    }

    async fn peers(&self, ctx: &Ctx, req: GetPeerParams) -> Option<GetPeers> {
        let peers = ctx
            .storage
            .peers(req.room, req.page as usize, req.limit as usize)
            .await?
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
        Some(GetPeers { peers })
    }

    async fn sessions(&self, ctx: &Ctx, req: GetParams) -> Option<GetSessions> {
        let sessions = ctx
            .storage
            .sessions(req.page as usize, req.limit as usize)
            .await?
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
        Some(GetSessions { sessions })
    }

    async fn events(&self, ctx: &Ctx, req: GetEventParams) -> Option<GetEvents> {
        let events = ctx
            .storage
            .events(req.session, req.start_ts, req.end_ts, req.page as usize, req.limit as usize)
            .await?
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
        Some(GetEvents { events })
    }
}
