use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "session")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub created_at: i64,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub sdk: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::event::Entity")]
    Events,
    #[sea_orm(has_many = "super::peer_session::Entity")]
    Peers,
}

impl Related<super::session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Events.def()
    }
}

impl Related<super::peer_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Peers.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
