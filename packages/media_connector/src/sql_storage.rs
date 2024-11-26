use std::{
    collections::{HashMap, VecDeque},
    time::Duration,
};

use atm0s_sdn::NodeId;
use media_server_protocol::{
    multi_tenancy::AppId,
    protobuf::cluster_connector::{
        connector_request, connector_response, hook_event, peer_event,
        record_event::{self, RecordPeerJoined, RecordStarted},
        room_event::{self, RoomAllPeersLeaved, RoomPeerJoined, RoomPeerLeaved, RoomStarted, RoomStopped},
        HookEvent, PeerRes, RecordEvent, RecordRes, RoomEvent,
    },
};
use media_server_utils::CustomUri;
use s3_presign::{Credentials, Presigner};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectOptions, Database, DatabaseConnection, DbErr, EntityTrait, FromQueryResult, JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    RelationTrait, Set,
};
use sea_orm_migration::MigratorTrait;
use sea_query::{Expr, IntoCondition};
use serde::Deserialize;

use crate::{ConnectorCfg, EventInfo, PagingResponse, PeerInfo, PeerSession, Querier, RoomInfo, SessionInfo, Storage};

mod entity;
mod migration;

#[derive(Deserialize, Clone)]
pub struct S3Options {
    pub path_style: Option<bool>,
    pub region: Option<String>,
}

pub struct ConnectorSqlStorage {
    node: NodeId,
    db: DatabaseConnection,
    s3: Presigner,
    s3_sub_folder: String,
    room_destroy_after_ms: u64,
    hook_events: VecDeque<(AppId, HookEvent)>,
}

impl ConnectorSqlStorage {
    pub async fn new(node: NodeId, cfg: &ConnectorCfg) -> Self {
        let mut opt = ConnectOptions::new(cfg.sql_uri.clone());
        opt.max_connections(100)
            .min_connections(5)
            .connect_timeout(Duration::from_secs(8))
            .acquire_timeout(Duration::from_secs(8))
            .idle_timeout(Duration::from_secs(8))
            .max_lifetime(Duration::from_secs(8))
            .sqlx_logging(true)
            .sqlx_logging_level(log::LevelFilter::Info);

        let db = Database::connect(opt).await.expect("Should connect to sql server");
        migration::Migrator::up(&db, None).await.expect("Should run migration success");

        let s3_endpoint = CustomUri::<S3Options>::try_from(cfg.s3_uri.as_str()).expect("should parse s3");
        let mut s3 = Presigner::new_with_root(
            Credentials::new(
                s3_endpoint.username.as_deref().expect("Should have s3 accesskey"),
                s3_endpoint.password.as_deref().expect("Should have s3 secretkey"),
                None,
            ),
            s3_endpoint.path.first().as_ref().expect("Should have bucket name"),
            s3_endpoint.query.region.as_ref().unwrap_or(&"".to_string()),
            s3_endpoint.host.as_str(),
        );
        if s3_endpoint.query.path_style == Some(true) {
            log::info!("[ConnectorSqlStorage] use path style");
            s3.use_path_style();
        }
        let s3_sub_folder = s3_endpoint.path[1..].join("/");

        Self {
            node,
            db,
            s3,
            s3_sub_folder,
            room_destroy_after_ms: cfg.room_destroy_after_ms,
            hook_events: Default::default(),
        }
    }

    async fn close_exited_rooms(&mut self, now_ms: u64) -> Result<(), DbErr> {
        let rooms_wait_destroy = entity::room::Entity::find()
            .filter(entity::room::Column::DestroyedAt.is_null())
            .filter(entity::room::Column::LastPeerLeavedAt.lte(now_ms - self.room_destroy_after_ms))
            .limit(100)
            .all(&self.db)
            .await?;
        log::info!("[ConnectorSqlStorage] clear {} rooms after {} ms inactive", rooms_wait_destroy.len(), self.room_destroy_after_ms);

        for room in rooms_wait_destroy {
            log::info!("[ConnectorSqlStorage] room {} {} no-one online after {}ms => destroy", room.id, room.room, self.room_destroy_after_ms);
            let app_id = room.app.clone();
            let room_name = room.room.clone();
            let mut model: entity::room::ActiveModel = room.into();
            model.destroyed_at = Set(Some(now_ms as i64));
            model.save(&self.db).await?;

            // all peers leave room => fire event
            self.hook_events.push_back((
                app_id.clone().into(),
                HookEvent {
                    node: self.node,
                    ts: now_ms,
                    event: Some(hook_event::Event::Room(RoomEvent {
                        app: app_id,
                        room: room_name,
                        event: Some(room_event::Event::Stopped(RoomStopped {})),
                    })),
                },
            ));
        }

        Ok(())
    }

    async fn on_peer_event(&mut self, now_ms: u64, from: NodeId, event_ts: u64, app: &str, session: u64, event: peer_event::Event) -> Result<(), DbErr> {
        if entity::session::Entity::find_by_id(session as i64).one(&self.db).await?.is_none() {
            log::info!("[ConnectorSqlStorage] new session {session} from node {from}");
            entity::session::Entity::insert(entity::session::ActiveModel {
                app: Set(app.to_owned()),
                id: Set(session as i64),
                created_at: Set(now_ms as i64),
                ip: ActiveValue::NotSet,
                sdk: ActiveValue::NotSet,
                user_agent: ActiveValue::NotSet,
            })
            .exec(&self.db)
            .await?;
        }

        match event {
            peer_event::Event::RouteBegin(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("RouteBegin".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;

                Ok(())
            }
            peer_event::Event::RouteSuccess(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("RouteSuccess".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::RouteError(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("RouteError".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::Connecting(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("Connecting".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::Connected(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("Connected".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::ConnectError(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("ConnectError".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::Stats(_) => todo!(),
            peer_event::Event::Reconnect(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("Reconnect".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::Reconnected(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("Reconnected".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::Disconnected(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("Disconnected".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::Join(params) => {
                let room = self.upsert_room(now_ms, event_ts, app, &params.room).await?;
                let peer = self.upsert_peer(now_ms, room, &params.peer).await?;
                let _peer_session = self.upsert_peer_session(now_ms, room, peer, session, event_ts).await?;

                // peer join room => fire event
                self.hook_events.push_back((
                    app.to_owned().into(),
                    HookEvent {
                        node: self.node,
                        ts: event_ts,
                        event: Some(hook_event::Event::Room(RoomEvent {
                            app: app.to_owned(),
                            room: params.room.clone(),
                            event: Some(room_event::Event::PeerJoined(RoomPeerJoined { peer: params.peer.clone() })),
                        })),
                    },
                ));

                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("Join".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::Leave(params) => {
                // peer leave room => fire event
                self.hook_events.push_back((
                    app.to_owned().into(),
                    HookEvent {
                        node: self.node,
                        ts: event_ts,
                        event: Some(hook_event::Event::Room(RoomEvent {
                            app: app.to_owned(),
                            room: params.room.clone(),
                            event: Some(room_event::Event::PeerLeaved(RoomPeerLeaved { peer: params.peer.clone() })),
                        })),
                    },
                ));

                let room = self.upsert_room(now_ms, event_ts, app, &params.room).await?;
                let peer = self.upsert_peer(now_ms, room, &params.peer).await?;
                let peer_session = entity::peer_session::Entity::find()
                    .filter(entity::peer_session::Column::Peer.eq(peer))
                    .filter(entity::peer_session::Column::Session.eq(session))
                    .one(&self.db)
                    .await?;
                if let Some(peer_session) = peer_session {
                    let mut model: entity::peer_session::ActiveModel = peer_session.into();
                    model.leaved_at = Set(Some(event_ts as i64));
                    model.save(&self.db).await?;
                }

                let online_peers = self.online_peers_count(room).await?;

                if online_peers == 0 {
                    log::info!("[ConnectorSqlStorage] last peer leaved room {}", params.room);
                    entity::room::Entity::update(entity::room::ActiveModel {
                        id: Set(room),
                        last_peer_leaved_at: Set(Some(now_ms as i64)),
                        ..Default::default()
                    })
                    .filter(entity::room::Column::Id.eq(room))
                    .exec(&self.db)
                    .await?;

                    // all peers leave room => fire event
                    self.hook_events.push_back((
                        app.to_owned().into(),
                        HookEvent {
                            node: self.node,
                            ts: event_ts,
                            event: Some(hook_event::Event::Room(RoomEvent {
                                app: app.to_owned(),
                                room: params.room.clone(),
                                event: Some(room_event::Event::AllPeersLeaved(RoomAllPeersLeaved {})),
                            })),
                        },
                    ));
                }

                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("Leave".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::RemoteTrackStarted(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("RemoteTrackStarted".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::RemoteTrackEnded(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("RemoteTrackEnded".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::LocalTrack(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("LocalTrack".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::LocalTrackAttach(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("LocalTrackAttach".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
            peer_event::Event::LocalTrackDetach(params) => {
                entity::event::ActiveModel {
                    id: ActiveValue::NotSet,
                    node: Set(from as i64),
                    node_ts: Set(event_ts as i64),
                    session: Set(session as i64),
                    created_at: Set(now_ms as i64),
                    event: Set("LocalTrackDetach".to_owned()),
                    meta: Set(Some(serde_json::to_value(params).expect("Should convert params to Json"))),
                }
                .insert(&self.db)
                .await?;
                Ok(())
            }
        }
    }

    async fn upsert_room(&mut self, now_ms: u64, event_ts: u64, app: &str, room: &str) -> Result<i32, DbErr> {
        let room_row = entity::room::Entity::find()
            .filter(entity::room::Column::App.eq(app))
            .filter(entity::room::Column::Room.eq(room))
            .filter(entity::room::Column::DestroyedAt.is_null())
            .one(&self.db)
            .await?;
        if let Some(info) = room_row {
            let room_id = info.id;
            if info.last_peer_leaved_at.is_some() {
                log::info!("[ConnectorSqlStorage] room {room} back to online => clear last_peer_leaved_at");
                let mut model: entity::room::ActiveModel = info.into();
                model.last_peer_leaved_at = Set(None);
                model.save(&self.db).await?;
            }

            Ok(room_id)
        } else {
            log::info!("[ConnectorSqlStorage] new room {room}");
            // new room created => fire event
            self.hook_events.push_back((
                app.to_owned().into(),
                HookEvent {
                    node: self.node,
                    ts: event_ts,
                    event: Some(hook_event::Event::Room(RoomEvent {
                        app: app.to_owned(),
                        room: room.to_owned(),
                        event: Some(room_event::Event::Started(RoomStarted {})),
                    })),
                },
            ));

            entity::room::ActiveModel {
                id: ActiveValue::NotSet,
                app: Set(app.to_owned()),
                room: Set(room.to_owned()),
                created_at: Set(now_ms as i64),
                ..Default::default()
            }
            .insert(&self.db)
            .await
            .map(|r| r.id)
        }
    }

    async fn upsert_peer(&self, now_ms: u64, room: i32, peer: &str) -> Result<i32, DbErr> {
        let peer_row = entity::peer::Entity::find()
            .filter(entity::peer::Column::Room.eq(room))
            .filter(entity::peer::Column::Peer.eq(peer))
            .one(&self.db)
            .await?;
        if let Some(info) = peer_row {
            Ok(info.id)
        } else {
            log::info!("[ConnectorSqlStorage] new peer {peer} joined internal_room_id {room}");
            entity::peer::ActiveModel {
                id: ActiveValue::NotSet,
                room: Set(room),
                peer: Set(peer.to_owned()),
                created_at: Set(now_ms as i64),
            }
            .insert(&self.db)
            .await
            .map(|r| r.id)
        }
    }

    async fn upsert_peer_session(&self, now_ms: u64, room: i32, peer: i32, session: u64, event_ts: u64) -> Result<i32, DbErr> {
        let peer_row = entity::peer_session::Entity::find()
            .filter(entity::peer_session::Column::Session.eq(session))
            .filter(entity::peer_session::Column::Peer.eq(peer))
            .filter(entity::peer_session::Column::LeavedAt.is_null())
            .one(&self.db)
            .await?;
        if let Some(info) = peer_row {
            Ok(info.id)
        } else {
            log::info!("[ConnectorSqlStorage] new peer_session {peer} for internal_room_id {room}");
            entity::peer_session::ActiveModel {
                id: ActiveValue::NotSet,
                session: Set(session as i64),
                peer: Set(peer),
                room: Set(room),
                created_at: Set(now_ms as i64),
                joined_at: Set(event_ts as i64),
                leaved_at: ActiveValue::NotSet,
                record: ActiveValue::NotSet,
            }
            .insert(&self.db)
            .await
            .map(|r| r.id)
        }
    }

    async fn online_peers_count(&self, room: i32) -> Result<u64, DbErr> {
        entity::peer_session::Entity::find()
            .filter(entity::peer_session::Column::LeavedAt.is_null())
            .join(
                JoinType::InnerJoin,
                entity::peer_session::Relation::Peer
                    .def()
                    .on_condition(move |_left, right| Expr::col((right, entity::peer::Column::Room)).eq(room).into_condition()),
            )
            .count(&self.db)
            .await
    }
}

impl Storage for ConnectorSqlStorage {
    type Q = ConnectorSqlQuerier;
    fn querier(&mut self) -> Self::Q {
        ConnectorSqlQuerier { db: self.db.clone() }
    }
    async fn on_tick(&mut self, now_ms: u64) {
        if let Err(e) = self.close_exited_rooms(now_ms).await {
            log::error!("[ConnectorSqlStorage] on_tick db error {e:?}");
        }
    }
    async fn on_event(&mut self, now_ms: u64, from: NodeId, event_ts: u64, event: connector_request::Request) -> Option<connector_response::Response> {
        match event {
            connector_request::Request::Peer(event) => {
                if let Err(e) = self.on_peer_event(now_ms, from, event_ts, &event.app, event.session_id, event.event.clone()?).await {
                    log::error!("[ConnectorSqlStorage] on_peer_event db error {e:?}");
                    return None;
                }
                self.hook_events.push_back((
                    event.app.clone().into(),
                    HookEvent {
                        node: from,
                        ts: event_ts,
                        event: Some(hook_event::Event::Peer(event)),
                    },
                ));
                Some(connector_response::Response::Peer(PeerRes {}))
            }
            connector_request::Request::Record(req) => {
                let room = entity::room::Entity::find()
                    .filter(entity::room::Column::App.eq(&req.app))
                    .filter(entity::room::Column::Room.eq(&req.room))
                    .filter(entity::room::Column::DestroyedAt.is_null())
                    .one(&self.db)
                    .await
                    .ok()??;
                let room_id = room.id;
                let room_path = if let Some(path) = room.record {
                    path
                } else {
                    let room_path = std::path::Path::new(&self.s3_sub_folder).join(&req.app).join(&req.room).join(room_id.to_string()).to_str()?.to_string();
                    log::info!("[ConnectorSqlStorage] room {} record started in path: {room_path}", req.room);
                    self.hook_events.push_back((
                        req.app.clone().into(),
                        HookEvent {
                            node: from,
                            ts: event_ts,
                            event: Some(hook_event::Event::Record(RecordEvent {
                                app: req.app.clone(),
                                room: req.room.clone(),
                                event: Some(record_event::Event::Started(RecordStarted { path: room_path.clone() })),
                            })),
                        },
                    ));

                    let mut model: entity::room::ActiveModel = room.into();
                    model.record = Set(Some(room_path.clone()));
                    model.save(&self.db).await.ok()?;
                    room_path
                };

                let peer_session = entity::peer_session::Entity::find()
                    .filter(entity::peer_session::Column::Session.eq(req.session as i64))
                    .filter(entity::peer_session::Column::Room.eq(room_id))
                    .order_by_desc(entity::peer_session::Column::CreatedAt)
                    .one(&self.db)
                    .await
                    .ok()??;
                let peer_path = if let Some(path) = peer_session.record {
                    path
                } else {
                    let peer_path = std::path::Path::new(&req.peer).join(peer_session.id.to_string()).to_str()?.to_string();
                    log::info!("[ConnectorSqlStorage] room {} peer {} record started in path: {peer_path}", req.room, req.peer);
                    self.hook_events.push_back((
                        req.app.clone().into(),
                        HookEvent {
                            node: from,
                            ts: event_ts,
                            event: Some(hook_event::Event::Record(RecordEvent {
                                app: req.app.clone(),
                                room: req.room.clone(),
                                event: Some(record_event::Event::PeerJoined(RecordPeerJoined {
                                    peer: req.peer.clone(),
                                    path: peer_path.clone(),
                                })),
                            })),
                        },
                    ));

                    let mut model: entity::peer_session::ActiveModel = peer_session.into();
                    model.record = Set(Some(peer_path.clone()));
                    model.save(&self.db).await.ok()?;
                    peer_path
                };

                let path = std::path::Path::new(&room_path)
                    .join(peer_path)
                    .join(format!("{}-{}-{}.rec", req.index, req.from_ts, req.to_ts))
                    .to_str()?
                    .to_string();
                let s3_uri = self.s3.put(&path, 86400).expect("Should create s3_uri");
                Some(connector_response::Response::Record(RecordRes { s3_uri }))
            }
        }
    }

    fn pop_hook_event(&mut self) -> Option<(AppId, HookEvent)> {
        self.hook_events.pop_front()
    }
}

pub struct ConnectorSqlQuerier {
    db: DatabaseConnection,
}

#[derive(FromQueryResult)]
struct RoomInfoAndPeersCount {
    pub id: i32,
    pub app: String,
    pub room: String,
    pub created_at: i64,
    pub destroyed_at: Option<i64>,
    pub peers: i64,
    pub record: Option<String>,
}

#[async_trait::async_trait]
impl Querier for ConnectorSqlQuerier {
    async fn rooms(&self, page: usize, limit: usize) -> Result<PagingResponse<RoomInfo>, String> {
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
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|r| RoomInfo {
                id: r.id,
                app: r.app,
                room: r.room,
                created_at: r.created_at as u64,
                destroyed_at: r.destroyed_at.map(|t| t as u64),
                peers: r.peers as usize,
                record: r.record,
            })
            .collect::<Vec<_>>();
        let total = entity::room::Entity::find().count(&self.db).await.map_err(|e| e.to_string())?;
        Ok(PagingResponse {
            data: rooms,
            current: page,
            total: calc_page_num(total as usize, limit),
        })
    }

    async fn peers(&self, room: Option<i32>, page: usize, limit: usize) -> Result<PagingResponse<PeerInfo>, String> {
        let peers = entity::peer::Entity::find();
        let peers = if let Some(room) = room {
            peers.filter(entity::peer::Column::Room.eq(room))
        } else {
            peers
        };

        let total = peers.clone().count(&self.db).await.map_err(|e| e.to_string())?;
        let peers = peers
            .order_by(entity::peer::Column::CreatedAt, sea_orm::Order::Desc)
            .find_with_related(entity::room::Entity)
            .group_by(entity::peer::Column::Id)
            .group_by(entity::room::Column::Id)
            .limit(limit as u64)
            .offset((page * limit) as u64)
            .all(&self.db)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect::<Vec<_>>();

        // TODO optimize this sub queries
        // should combine into single query but it not allowed by sea-orm with multiple find_with_related
        let peer_ids = peers.iter().map(|(p, _)| p.id).collect::<Vec<_>>();
        let peer_sessions = entity::peer_session::Entity::find()
            .filter(entity::peer_session::Column::Peer.is_in(peer_ids))
            .all(&self.db)
            .await
            .map_err(|e| e.to_string())?;
        let mut peers_sessions_map = HashMap::new();
        for peer_session in peer_sessions {
            let entry = peers_sessions_map.entry(peer_session.peer).or_insert(vec![]);
            entry.push(peer_session);
        }

        Ok(PagingResponse {
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

    async fn sessions(&self, page: usize, limit: usize) -> Result<PagingResponse<SessionInfo>, String> {
        let sessions = entity::session::Entity::find()
            .order_by(entity::session::Column::CreatedAt, sea_orm::Order::Desc)
            .limit(limit as u64)
            .offset((page * limit) as u64)
            .find_with_related(entity::peer_session::Entity)
            .all(&self.db)
            .await
            .map_err(|e| e.to_string())?;
        let total = entity::session::Entity::find().count(&self.db).await.map_err(|e| e.to_string())?;

        // TODO optimize this sub queries
        // should combine into single query but it not allowed by sea-orm with multiple find_with_related
        let peers_id = sessions.iter().flat_map(|(_, peers)| peers.iter().map(|p| p.peer)).collect::<Vec<_>>();
        let peers = entity::peer::Entity::find()
            .filter(entity::peer::Column::Id.is_in(peers_id))
            .all(&self.db)
            .await
            .map_err(|e| e.to_string())?;
        let mut peers_map = HashMap::new();
        for peer in peers {
            peers_map.insert(peer.id, peer);
        }

        Ok(PagingResponse {
            data: sessions
                .into_iter()
                .map(|(r, peers)| SessionInfo {
                    id: r.id as u64,
                    app: r.app,
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

    async fn events(&self, session: Option<u64>, from: Option<u64>, to: Option<u64>, page: usize, limit: usize) -> Result<PagingResponse<EventInfo>, String> {
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

        let total = events.clone().count(&self.db).await.map_err(|e| e.to_string())?;
        let events = events
            .order_by(entity::event::Column::CreatedAt, sea_orm::Order::Desc)
            .limit(limit as u64)
            .offset((page * limit) as u64)
            .all(&self.db)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|r| EventInfo {
                id: r.id,
                node: r.node as u32,
                created_at: r.created_at as u64,
                session: r.session as u64,
                node_ts: r.node_ts as u64,
                event: r.event,
                meta: r.meta,
            })
            .collect::<Vec<_>>();

        Ok(PagingResponse {
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
        connector_request, hook_event,
        peer_event::{Connected, Connecting, Event, Join, Leave, RouteBegin},
        room_event::{self, RoomAllPeersLeaved, RoomPeerJoined, RoomPeerLeaved, RoomStarted, RoomStopped},
        HookEvent, PeerEvent, RoomEvent,
    };

    use crate::{ConnectorCfg, HookBodyType, Querier, Storage};

    use super::{calc_page_num, ConnectorSqlStorage};

    #[tokio::test]
    async fn test_event() {
        let app = "app1";
        let session_id = 10000;
        let node = 1;
        let ts = 1000;
        let remote_ip = "127.0.0.1".to_string();
        let cfg = ConnectorCfg {
            sql_uri: "sqlite::memory:".to_owned(),
            s3_uri: "http://user:pass@localhost:9000/bucket".to_owned(),
            hook_workers: 0,
            hook_body_type: HookBodyType::ProtobufJson,
            room_destroy_after_ms: 300_000,
        };
        let mut storage = ConnectorSqlStorage::new(node, &cfg).await;
        let querier = storage.querier();
        let event = PeerEvent {
            app: app.to_owned(),
            session_id,
            event: Some(Event::RouteBegin(RouteBegin { remote_ip: remote_ip.clone() })),
        };
        storage.on_event(0, node, ts, connector_request::Request::Peer(event.clone())).await.expect("Should process event");

        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Peer(event))
                }
            ))
        );
        assert_eq!(storage.pop_hook_event(), None);

        let sessions = querier.sessions(0, 2).await.expect("Should got sessions");
        assert_eq!(sessions.data.len(), 1);
        assert_eq!(sessions.total, 1);
        assert_eq!(sessions.current, 0);

        let events = querier.events(None, None, None, 0, 2).await.expect("Should got events");
        assert_eq!(events.data.len(), 1);
        assert_eq!(events.total, 1);
        assert_eq!(events.current, 0);

        let session_events = querier.events(Some(session_id), None, None, 0, 2).await.expect("Should got events");
        assert_eq!(session_events.data.len(), 1);
        assert_eq!(session_events.total, 1);
        assert_eq!(session_events.current, 0);
    }

    #[tokio::test]
    async fn test_room() {
        let app = "app1";
        let session_id = 10000;
        let node = 1;
        let ts = 1000;
        let remote_ip = "127.0.0.1".to_string();
        let cfg = ConnectorCfg {
            sql_uri: "sqlite::memory:".to_owned(),
            s3_uri: "http://user:pass@localhost:9000/bucket".to_owned(),
            hook_workers: 0,
            hook_body_type: HookBodyType::ProtobufJson,
            room_destroy_after_ms: 300_000,
        };
        let mut storage = ConnectorSqlStorage::new(node, &cfg).await;
        let querier = storage.querier();
        let connecting_event = PeerEvent {
            app: app.to_owned(),
            session_id,
            event: Some(Event::Connecting(Connecting { remote_ip: remote_ip.clone() })),
        };
        storage
            .on_event(0, node, ts, connector_request::Request::Peer(connecting_event.clone()))
            .await
            .expect("Should process event");

        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Peer(connecting_event))
                }
            ))
        );
        assert_eq!(storage.pop_hook_event(), None);

        let sessions = querier.sessions(0, 2).await.expect("Should got sessions");
        assert_eq!(sessions.data.len(), 1);
        assert_eq!(sessions.total, 1);
        assert_eq!(sessions.current, 0);

        let events = querier.events(None, None, None, 0, 2).await.expect("Should got events");
        assert_eq!(events.data.len(), 1);
        assert_eq!(events.total, 1);
        assert_eq!(events.current, 0);

        let session_events = querier.events(Some(session_id), None, None, 0, 2).await.expect("Should got events");
        assert_eq!(session_events.data.len(), 1);
        assert_eq!(session_events.total, 1);
        assert_eq!(session_events.current, 0);

        let connected_event = PeerEvent {
            app: app.to_owned(),
            session_id,
            event: Some(Event::Connected(Connected {
                after_ms: 10,
                remote_ip: remote_ip.clone(),
            })),
        };
        storage
            .on_event(0, node, ts, connector_request::Request::Peer(connected_event.clone()))
            .await
            .expect("Should process event");
        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Peer(connected_event))
                }
            ))
        );
        assert_eq!(storage.pop_hook_event(), None);

        let rooms = querier.rooms(0, 2).await.expect("Should got rooms");
        assert_eq!(rooms.data.len(), 0);
        assert_eq!(rooms.total, 0);
        assert_eq!(rooms.current, 0);

        let join_event = PeerEvent {
            app: app.to_owned(),
            session_id,
            event: Some(Event::Join(Join {
                room: "demo".to_string(),
                peer: "peer".to_string(),
            })),
        };
        storage.on_event(0, node, ts, connector_request::Request::Peer(join_event.clone())).await.expect("Should process event");

        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Room(RoomEvent {
                        app: app.to_owned(),
                        room: "demo".to_owned(),
                        event: Some(room_event::Event::Started(RoomStarted {}))
                    }))
                }
            ))
        );
        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Room(RoomEvent {
                        app: app.to_owned(),
                        room: "demo".to_owned(),
                        event: Some(room_event::Event::PeerJoined(RoomPeerJoined { peer: "peer".to_owned() }))
                    }))
                }
            ))
        );
        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Peer(join_event))
                }
            ))
        );
        assert_eq!(storage.pop_hook_event(), None);

        let rooms = querier.rooms(0, 2).await.expect("Should got rooms");
        assert_eq!(rooms.data.len(), 1);
        assert_eq!(rooms.total, 1);
        assert_eq!(rooms.current, 0);

        let peers = querier.peers(None, 0, 2).await.expect("Should got peers");
        assert_eq!(peers.data.len(), 1);
        assert_eq!(peers.total, 1);
        assert_eq!(peers.current, 0);

        // now leave room
        let leave_event = PeerEvent {
            app: app.to_owned(),
            session_id,
            event: Some(Event::Leave(Leave {
                room: "demo".to_string(),
                peer: "peer".to_string(),
            })),
        };
        storage
            .on_event(1000, node, ts, connector_request::Request::Peer(leave_event.clone()))
            .await
            .expect("Should process event");

        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Room(RoomEvent {
                        app: app.to_owned(),
                        room: "demo".to_string(),
                        event: Some(room_event::Event::PeerLeaved(RoomPeerLeaved { peer: "peer".to_string() }))
                    }))
                }
            ))
        );
        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Room(RoomEvent {
                        app: app.to_owned(),
                        room: "demo".to_owned(),
                        event: Some(room_event::Event::AllPeersLeaved(RoomAllPeersLeaved {}))
                    }))
                }
            ))
        );
        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Peer(leave_event))
                }
            ))
        );
        assert_eq!(storage.pop_hook_event(), None);

        // we will destroy room after timeout
        storage.on_tick(1000 + cfg.room_destroy_after_ms).await;

        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts: 1000 + cfg.room_destroy_after_ms,
                    event: Some(hook_event::Event::Room(RoomEvent {
                        app: app.to_owned(),
                        room: "demo".to_owned(),
                        event: Some(room_event::Event::Stopped(RoomStopped {}))
                    }))
                }
            ))
        );
        assert_eq!(storage.pop_hook_event(), None);

        // now we will create new room
        let ts = ts + 10000;
        let join_event = PeerEvent {
            app: app.to_owned(),
            session_id,
            event: Some(Event::Join(Join {
                room: "demo".to_string(),
                peer: "peer".to_string(),
            })),
        };
        storage.on_event(0, node, ts, connector_request::Request::Peer(join_event.clone())).await.expect("Should process event");
        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Room(RoomEvent {
                        app: app.to_owned(),
                        room: "demo".to_owned(),
                        event: Some(room_event::Event::Started(RoomStarted {}))
                    }))
                }
            ))
        );
        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Room(RoomEvent {
                        app: app.to_owned(),
                        room: "demo".to_owned(),
                        event: Some(room_event::Event::PeerJoined(RoomPeerJoined { peer: "peer".to_owned() }))
                    }))
                }
            ))
        );
        assert_eq!(
            storage.pop_hook_event(),
            Some((
                app.to_owned().into(),
                HookEvent {
                    node,
                    ts,
                    event: Some(hook_event::Event::Peer(join_event))
                }
            ))
        );
        assert_eq!(storage.pop_hook_event(), None);

        let rooms = querier.rooms(0, 3).await.expect("Should got rooms");
        assert_eq!(rooms.data.len(), 2);
        assert_eq!(rooms.total, 1);
        assert_eq!(rooms.current, 0);

        let peers = querier.peers(None, 0, 3).await.expect("Should got peers");
        assert_eq!(peers.data.len(), 2);
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
