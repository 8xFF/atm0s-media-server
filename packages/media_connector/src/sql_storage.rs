use std::time::Duration;

use atm0s_sdn::NodeId;
use media_server_protocol::protobuf::cluster_connector::{connector_request, peer_event};
use media_server_utils::now_ms;
use sea_orm::{sea_query::OnConflict, ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set};
use sea_orm_migration::MigratorTrait;

use crate::{EventInfo, Querier, RoomInfo, SessionInfo, Storage};

mod entity;
mod migration;

pub struct ConnectorStorage {
    db: DatabaseConnection,
}

impl ConnectorStorage {
    pub async fn new(sql_uri: &str) -> Self {
        let mut opt = ConnectOptions::new(sql_uri.to_owned());
        opt.max_connections(100)
            .min_connections(5)
            .connect_timeout(Duration::from_secs(8))
            .acquire_timeout(Duration::from_secs(8))
            .idle_timeout(Duration::from_secs(8))
            .max_lifetime(Duration::from_secs(8))
            .sqlx_logging(true)
            .sqlx_logging_level(log::LevelFilter::Info); // Setting default PostgreSQL schema

        let db = Database::connect(opt).await.expect("Should connect to sql server");
        migration::Migrator::up(&db, None).await.expect("Should run migration success");

        Self { db }
    }

    async fn on_peer_event(&self, from: NodeId, ts: u64, session: u64, event: peer_event::Event) -> Option<()> {
        match event {
            peer_event::Event::RouteBegin(params) => {
                entity::session::Entity::insert(entity::session::ActiveModel {
                    id: Set(session as i64),
                    created_at: Set(now_ms() as i64),
                    ip: Set(Some(params.ip_addr.clone())),
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
                    ip: Set(Some(params.ip_addr.clone())),
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
    async fn on_event(&self, from: NodeId, ts: u64, req_id: u64, event: connector_request::Event) -> Option<()> {
        match event {
            connector_request::Event::Peer(event) => self.on_peer_event(from, ts, event.session_id, event.event?).await,
        }
    }
}

impl Querier for ConnectorStorage {
    async fn rooms(&self, page: usize, count: usize) -> Option<Vec<RoomInfo>> {
        let rooms = entity::room::Entity::find()
            .limit(count as u64)
            .offset((page * count) as u64)
            .all(&self.db)
            .await
            .ok()?
            .into_iter()
            .map(|r| RoomInfo {
                id: r.id,
                room: r.room,
                created_at: r.created_at as u64,
                peers: 0, //TODO count peers
            })
            .collect::<Vec<_>>();
        Some(rooms)
    }

    async fn room(&self, room: i32) -> Option<crate::RoomInfo> {
        todo!()
    }

    async fn peers(&self, room: Option<i32>, page: usize, count: usize) -> Option<Vec<crate::PeerInfo>> {
        todo!()
    }

    async fn peer(&self, peer: i32) -> Option<crate::PeerInfo> {
        todo!()
    }

    async fn sessions(&self, peer: Option<i32>, page: usize, count: usize) -> Option<Vec<SessionInfo>> {
        let sessions = entity::session::Entity::find()
            .limit(count as u64)
            .offset((page * count) as u64)
            .all(&self.db)
            .await
            .ok()?
            .into_iter()
            .map(|r| SessionInfo {
                id: r.id as u64,
                created_at: r.created_at as u64,
                ip: r.ip,
                user_agent: r.user_agent,
                sdk: r.sdk,
                events: 0, //TODO count events
            })
            .collect::<Vec<_>>();
        Some(sessions)
    }

    async fn session(&self, session: u64) -> Option<crate::SessionInfo> {
        todo!()
    }

    async fn events(&self, session: Option<i32>, page: usize, count: usize) -> Option<Vec<EventInfo>> {
        let events = entity::event::Entity::find()
            .limit(count as u64)
            .offset((page * count) as u64)
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
        Some(events)
    }
}
