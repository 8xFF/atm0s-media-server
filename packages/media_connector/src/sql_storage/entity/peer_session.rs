use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "peer_session")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub peer: i32,
    pub session: i64,
    pub created_at: i64,
    /// This is node timestamp
    pub joined_at: i64,
    /// This is node timestamp
    pub leaved_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(belongs_to = "super::peer::Entity", from = "Column::Peer", to = "super::peer::Column::Id")]
    Peer,
}

impl Related<super::peer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Peer.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
