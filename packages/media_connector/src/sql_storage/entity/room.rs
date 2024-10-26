use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "room")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub app: String,
    pub room: String,
    /// Record folder path
    pub record: Option<String>,
    pub created_at: i64,
    /// This is node timestamp
    pub last_peer_leaved_at: Option<i64>,
    /// This is node timestamp
    pub destroyed_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::peer::Entity")]
    Peers,
    #[sea_orm(has_many = "super::peer_session::Entity")]
    PeerSessions,
}

impl Related<super::peer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Peers.def()
    }
}

impl Related<super::peer_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PeerSessions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
