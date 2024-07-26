use std::{collections::HashMap, time::Duration};

use atm0s_sdn::NodeId;
use media_server_protocol::protobuf::cluster_connector::{connector_request, connector_response, peer_event, PeerRes, RecordRes};
use media_server_utils::{now_ms, CustomUri};
use s3_presign::{Credentials, Presigner};
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, FromQueryResult, JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    RelationTrait, Set,
};
use sea_orm_migration::MigratorTrait;
use serde::Deserialize;

use crate::{EventInfo, PagingResponse, PeerInfo, PeerSession, Querier, RoomInfo, SessionInfo, Storage};

mod entity;
mod migration;

#[derive(Deserialize, Clone)]
pub struct S3Options {
    pub path_style: Option<bool>,
    pub region: Option<String>,
}

pub struct ConnectorStorage {
    db: DatabaseConnection,
    s3: Presigner,
    s3_sub_folder: String,
}

impl ConnectorStorage {
    pub async fn new(sql_uri: &str, s3_uri: &str) -> Self {
        let mut opt = ConnectOptions::new(sql_uri.to_owned());
        opt.max_connections(100)
            .min_connections(5)
            .connect_timeout(Duration::from_secs(8))
            .acquire_timeout(Duration::from_secs(8))
            .idle_timeout(Duration::from_secs(8))
            .max_lifetime(Duration::from_secs(8))
            .sqlx_logging(false)
            .sqlx_logging_level(log::LevelFilter::Info); // Setting default PostgreSQL schema

        let db = Database::connect(opt).await.expect("Should connect to sql server");
        migration::Migrator::up(&db, None).await.expect("Should run migration success");

        let s3_endpoint = CustomUri::<S3Options>::try_from(s3_uri).expect("should parse s3");
        let mut s3 = Presigner::new(
            Credentials::new(s3_endpoint.username.expect("Should have s3 accesskey"), s3_endpoint.password.expect("Should have s3 secretkey"), None),
            s3_endpoint.path.first().as_ref().expect("Should have bucket name"),
            s3_endpoint.query.region.as_ref().unwrap_or(&"".to_string()),
        );
        s3.endpoint(s3_endpoint.endpoint.as_str());
        if s3_endpoint.query.path_style == Some(true) {
            s3.use_path_style();
        }

        let s3_sub_folder = s3_endpoint.path[1..].join("/");

        Self { db, s3, s3_sub_folder }
    }

    async fn on_peer_event(&self, from: NodeId, ts: u64, session: u64, event: peer_event::Event) -> Option<()> {
        match event {
            peer_event::Event::RouteBegin(params) => {
                entity::session::Entity::insert(entity::session::ActiveModel {
                    id: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    ip: Set(Some(params.remote_ip.clone())),
                    ..Default::default()
                })
                .exec(&self.db)
                .await
                .ok()?;

                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("RouteBegin".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;

                Some(())
            }
            peer_event::Event::RouteSuccess(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("RouteSuccess".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::RouteError(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("RouteError".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::Connecting(params) => {
                entity::session::Entity::insert(entity::session::ActiveModel {
                    id: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    ..Default::default()
                })
                .on_conflict(
                    // on conflict do nothing
                    OnConflict::column(entity::session::Column::Id).do_nothing().to_owned(),
                )
                .exec(&self.db)
                .await
                .ok()?;

                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("Connecting".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::Connected(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("Connected".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::ConnectError(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("ConnectError".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::Stats(_) => todo!(),
            peer_event::Event::Reconnect(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("Reconnect".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::Reconnected(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("Reconnected".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::Disconnected(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("Disconnected".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::Join(params) => {
                let room = self.upsert_room(&params.room).await?;
                let peer = self.upsert_peer(room, &params.peer).await?;
                let _peer_session = self.upsert_peer_session(peer, session, ts).await?;

                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("Join".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::Leave(params) => {
                let room = self.upsert_room(&params.room).await?;
                let peer = self.upsert_peer(room, &params.peer).await?;
                let peer_session = entity::peer_session::Entity::find()
                    .filter(entity::peer_session::Column::Peer.eq(peer))
                    .filter(entity::peer_session::Column::Session.eq(session))
                    .one(&self.db)
                    .await
                    .ok()?;
                if let Some(peer_session) = peer_session {
                    let mut model: entity::peer_session::ActiveModel = peer_session.into();
                    model.leaved_at = Set(Some(ts as i64));
                    model.save(&self.db).await.ok()?;
                }

                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("Leave".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::RemoteTrackStarted(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("RemoteTrackStarted".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::RemoteTrackEnded(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("RemoteTrackEnded".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::LocalTrack(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("LocalTrack".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::LocalTrackAttach(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("LocalTrackAttach".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
            peer_event::Event::LocalTrackDetach(params) => {
                entity::event::ActiveModel {
                    node: Set(from),
                    node_ts: Set(ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    event: Set("LocalTrackDetach".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                    ..Default::default()
                }
                .insert(&self.db)
                .await
                .ok()?;
                Some(())
            }
        }
    }

    async fn upsert_room(&self, room: &str) -> Option<i32> {
        let room_row = entity::room::Entity::find().filter(entity::room::Column::Room.eq(room)).one(&self.db).await.ok()?;
        if let Some(info) = room_row {
            Some(info.id)
        } else {
            entity::room::ActiveModel {
                room: Set(room.to_owned()),
                created_at: Set(now_ms() as i64),
                ..Default::default()
            }
            .insert(&self.db)
            .await
            .ok()
            .map(|r| r.id)
        }
    }

    async fn upsert_peer(&self, room: i32, peer: &str) -> Option<i32> {
        let peer_row = entity::peer::Entity::find()
            .filter(entity::peer::Column::Room.eq(room))
            .filter(entity::peer::Column::Peer.eq(peer))
            .one(&self.db)
            .await
            .ok()?;
        if let Some(info) = peer_row {
            Some(info.id)
        } else {
            entity::peer::ActiveModel {
                room: Set(room),
                peer: Set(peer.to_owned()),
                created_at: Set(now_ms() as i64),
                ..Default::default()
            }
            .insert(&self.db)
            .await
            .ok()
            .map(|r| r.id)
        }
    }

    async fn upsert_peer_session(&self, peer: i32, session: u64, ts: u64) -> Option<i32> {
        let peer_row = entity::peer_session::Entity::find()
            .filter(entity::peer_session::Column::Session.eq(session))
            .filter(entity::peer_session::Column::Peer.eq(peer))
            .one(&self.db)
            .await
            .ok()?;
        if let Some(info) = peer_row {
            Some(info.id)
        } else {
            entity::peer_session::ActiveModel {
                session: Set(session as i64),
                peer: Set(peer),
                created_at: Set(now_ms() as i64),
                joined_at: Set(ts as i64),
                ..Default::default()
            }
            .insert(&self.db)
            .await
            .ok()
            .map(|r| r.id)
        }
    }
}

impl Storage for ConnectorStorage {
    async fn on_event(&self, from: NodeId, ts: u64, event: connector_request::Request) -> Option<connector_response::Response> {
        match event {
            connector_request::Request::Peer(event) => {
                self.on_peer_event(from, ts, event.session_id, event.event?).await;
                Some(connector_response::Response::Peer(PeerRes {}))
            }
            connector_request::Request::Record(req) => {
                let path = std::path::Path::new(&self.s3_sub_folder)
                    .join(req.room)
                    .join(req.peer)
                    .join(req.session.to_string())
                    .join(format!("{}-{}-{}.rec", req.index, req.from_ts, req.to_ts))
                    .to_str()?
                    .to_string();
                let s3_uri = self.s3.put(&path, 86400).expect("Should create s3_uri");
                Some(connector_response::Response::Record(RecordRes { s3_uri }))
            }
        }
    }
}

#[derive(FromQueryResult)]
struct RoomInfoAndPeersCount {
    pub id: i32,
    pub room: String,
    pub created_at: i64,
    pub peers: i32,
}

impl Querier for ConnectorStorage {
    async fn rooms(&self, page: usize, limit: usize) -> Option<PagingResponse<RoomInfo>> {
        let rooms = entity::room::Entity::find()
            .column_as(entity::peer::Column::Id.count(), "peers")
            .join_rev(JoinType::LeftJoin, entity::peer::Relation::Room.def())
            .group_by(entity::room::Column::Id)
            .order_by(entity::room::Column::CreatedAt, sea_orm::Order::Desc)
            .limit(limit as u64)
            .offset((page * limit) as u64)
            .into_model::<RoomInfoAndPeersCount>()
            .all(&self.db)
            .await
            .ok()?
            .into_iter()
            .map(|r| RoomInfo {
                id: r.id,
                room: r.room,
                created_at: r.created_at as u64,
                peers: r.peers as usize,
            })
            .collect::<Vec<_>>();
        let total = entity::room::Entity::find().count(&self.db).await.ok()?;
        Some(PagingResponse {
            data: rooms,
            current: page,
            total: calc_page_num(total as usize, limit),
        })
    }

    async fn peers(&self, room: Option<i32>, page: usize, limit: usize) -> Option<PagingResponse<PeerInfo>> {
        let peers = entity::peer::Entity::find();
        let peers = if let Some(room) = room {
            peers.filter(entity::peer::Column::Room.eq(room))
        } else {
            peers
        };

        let total = peers.clone().count(&self.db).await.ok()?;
        let peers = peers
            .order_by(entity::peer::Column::CreatedAt, sea_orm::Order::Desc)
            .find_with_related(entity::room::Entity)
            .group_by(entity::peer::Column::Id)
            .limit(limit as u64)
            .offset((page * limit) as u64)
            .all(&self.db)
            .await
            .ok()?
            .into_iter()
            .collect::<Vec<_>>();

        // TODO optimize this sub queries
        // should combine into single query but it not allowed by sea-orm with multiple find_with_related
        let peer_ids = peers.iter().map(|(p, _)| p.id).collect::<Vec<_>>();
        let peer_sessions = entity::peer_session::Entity::find()
            .filter(entity::peer_session::Column::Peer.is_in(peer_ids))
            .all(&self.db)
            .await
            .ok()?;
        let mut peers_sessions_map = HashMap::new();
        for peer_session in peer_sessions {
            let entry = peers_sessions_map.entry(peer_session.peer).or_insert(vec![]);
            entry.push(peer_session);
        }

        Some(PagingResponse {
            data: peers
                .into_iter()
                .map(|(peer, room)| PeerInfo {
                    id: peer.id,
                    room_id: peer.room,
                    room: room.first().map(|r| r.room.clone()).unwrap_or("".to_string()),
                    peer: peer.peer.clone(),
                    created_at: peer.created_at as u64,
                    sessions: peers_sessions_map
                        .remove(&peer.id)
                        .into_iter()
                        .flatten()
                        .map(|s| PeerSession {
                            id: s.id,
                            peer_id: s.peer,
                            peer: peer.peer.clone(),
                            session: s.session as u64,
                            created_at: s.created_at as u64,
                            joined_at: s.joined_at as u64,
                            leaved_at: s.leaved_at.map(|l| l as u64),
                        })
                        .collect::<Vec<_>>(),
                })
                .collect::<Vec<_>>(),
            current: page,
            total: calc_page_num(total as usize, limit),
        })
    }

    async fn sessions(&self, page: usize, limit: usize) -> Option<PagingResponse<SessionInfo>> {
        let sessions = entity::session::Entity::find()
            .order_by(entity::session::Column::CreatedAt, sea_orm::Order::Desc)
            .limit(limit as u64)
            .offset((page * limit) as u64)
            .find_with_related(entity::peer_session::Entity)
            .all(&self.db)
            .await
            .ok()?;
        let total = entity::session::Entity::find().count(&self.db).await.ok()?;

        // TODO optimize this sub queries
        // should combine into single query but it not allowed by sea-orm with multiple find_with_related
        let peers_id = sessions.iter().flat_map(|(_, peers)| peers.iter().map(|p| p.peer)).collect::<Vec<_>>();
        let peers = entity::peer::Entity::find().filter(entity::peer::Column::Id.is_in(peers_id)).all(&self.db).await.ok()?;
        let mut peers_map = HashMap::new();
        for peer in peers {
            peers_map.insert(peer.id, peer);
        }

        Some(PagingResponse {
            data: sessions
                .into_iter()
                .map(|(r, peers)| SessionInfo {
                    id: r.id as u64,
                    created_at: r.created_at as u64,
                    ip: r.ip,
                    user_agent: r.user_agent,
                    sdk: r.sdk,
                    peers: peers
                        .into_iter()
                        .map(|s| PeerSession {
                            id: s.id,
                            peer_id: s.peer,
                            peer: peers_map.get(&s.peer).map(|p| p.peer.clone()).unwrap_or("_".to_string()),
                            session: s.session as u64,
                            created_at: s.created_at as u64,
                            joined_at: s.joined_at as u64,
                            leaved_at: s.leaved_at.map(|l| l as u64),
                        })
                        .collect::<Vec<_>>(),
                })
                .collect::<Vec<_>>(),
            current: page,
            total: calc_page_num(total as usize, limit),
        })
    }

    async fn events(&self, session: Option<u64>, from: Option<u64>, to: Option<u64>, page: usize, limit: usize) -> Option<PagingResponse<EventInfo>> {
        let events = entity::event::Entity::find();
        let events = if let Some(session) = session {
            events.filter(entity::event::Column::Session.eq(session as i64))
        } else {
            events
        };

        let events = if let Some(from) = from {
            events.filter(entity::event::Column::CreatedAt.gte(from as i64))
        } else {
            events
        };

        let events = if let Some(to) = to {
            events.filter(entity::event::Column::CreatedAt.lte(to as i64))
        } else {
            events
        };

        let total = events.clone().count(&self.db).await.ok()?;
        let events = events
            .order_by(entity::event::Column::CreatedAt, sea_orm::Order::Desc)
            .limit(limit as u64)
            .offset((page * limit) as u64)
            .all(&self.db)
            .await
            .unwrap()
            .into_iter()
            .map(|r| EventInfo {
                id: r.id,
                node: r.node,
                created_at: r.created_at as u64,
                session: r.session as u64,
                node_ts: r.node_ts as u64,
                event: r.event,
                meta: r.meta,
            })
            .collect::<Vec<_>>();
        Some(PagingResponse {
            data: events,
            current: page,
            total: calc_page_num(total as usize, limit),
        })
    }
}

fn calc_page_num(elms: usize, page_size: usize) -> usize {
    if elms == 0 {
        0
    } else {
        1 + (elms - 1) / page_size
    }
}

#[cfg(test)]
mod tests {
    use media_server_protocol::protobuf::cluster_connector::{
        connector_request,
        peer_event::{Connected, Connecting, Event, Join, RouteBegin},
        PeerEvent,
    };

    use crate::{Querier, Storage};

    use super::{calc_page_num, ConnectorStorage};

    #[tokio::test]
    async fn test_event() {
        let session_id = 10000;
        let node = 1;
        let ts = 1000;
        let remote_ip = "127.0.0.1".to_string();
        let storage = ConnectorStorage::new("sqlite::memory:", "http://user:pass@localhost:9000/bucket").await;
        storage
            .on_event(
                node,
                ts,
                connector_request::Request::Peer(PeerEvent {
                    session_id,
                    event: Some(Event::RouteBegin(RouteBegin { remote_ip: remote_ip.clone() })),
                }),
            )
            .await
            .expect("Should process event");

        let sessions = storage.sessions(0, 2).await.expect("Should got sessions");
        assert_eq!(sessions.data.len(), 1);
        assert_eq!(sessions.total, 1);
        assert_eq!(sessions.current, 0);

        let events = storage.events(None, None, None, 0, 2).await.expect("Should got events");
        assert_eq!(events.data.len(), 1);
        assert_eq!(events.total, 1);
        assert_eq!(events.current, 0);

        let session_events = storage.events(Some(session_id), None, None, 0, 2).await.expect("Should got events");
        assert_eq!(session_events.data.len(), 1);
        assert_eq!(session_events.total, 1);
        assert_eq!(session_events.current, 0);
    }

    #[tokio::test]
    async fn test_room() {
        let session_id = 10000;
        let node = 1;
        let ts = 1000;
        let remote_ip = "127.0.0.1".to_string();
        let storage = ConnectorStorage::new("sqlite::memory:", "http://user:pass@localhost:9000/bucket").await;
        storage
            .on_event(
                node,
                ts,
                connector_request::Request::Peer(PeerEvent {
                    session_id,
                    event: Some(Event::Connecting(Connecting { remote_ip: remote_ip.clone() })),
                }),
            )
            .await
            .expect("Should process event");

        let sessions = storage.sessions(0, 2).await.expect("Should got sessions");
        assert_eq!(sessions.data.len(), 1);
        assert_eq!(sessions.total, 1);
        assert_eq!(sessions.current, 0);

        let events = storage.events(None, None, None, 0, 2).await.expect("Should got events");
        assert_eq!(events.data.len(), 1);
        assert_eq!(events.total, 1);
        assert_eq!(events.current, 0);

        let session_events = storage.events(Some(session_id), None, None, 0, 2).await.expect("Should got events");
        assert_eq!(session_events.data.len(), 1);
        assert_eq!(session_events.total, 1);
        assert_eq!(session_events.current, 0);

        storage
            .on_event(
                node,
                ts,
                connector_request::Request::Peer(PeerEvent {
                    session_id,
                    event: Some(Event::Connected(Connected {
                        after_ms: 10,
                        remote_ip: remote_ip.clone(),
                    })),
                }),
            )
            .await
            .expect("Should process event");

        let rooms = storage.rooms(0, 2).await.expect("Should got rooms");
        assert_eq!(rooms.data.len(), 0);
        assert_eq!(rooms.total, 0);
        assert_eq!(rooms.current, 0);

        storage
            .on_event(
                node,
                ts,
                connector_request::Request::Peer(PeerEvent {
                    session_id,
                    event: Some(Event::Join(Join {
                        room: "demo".to_string(),
                        peer: "peer".to_string(),
                    })),
                }),
            )
            .await
            .expect("Should process event");

        let rooms = storage.rooms(0, 2).await.expect("Should got rooms");
        assert_eq!(rooms.data.len(), 1);
        assert_eq!(rooms.total, 1);
        assert_eq!(rooms.current, 0);

        let peers = storage.peers(None, 0, 2).await.expect("Should got peers");
        assert_eq!(peers.data.len(), 1);
        assert_eq!(peers.total, 1);
        assert_eq!(peers.current, 0);
    }

    //TODO: test with record link generate

    #[test]
    fn test_calc_page_num() {
        assert_eq!(calc_page_num(0, 100), 0);
        assert_eq!(calc_page_num(1, 100), 1);
        assert_eq!(calc_page_num(99, 100), 1);
        assert_eq!(calc_page_num(100, 100), 1);
    }
}
