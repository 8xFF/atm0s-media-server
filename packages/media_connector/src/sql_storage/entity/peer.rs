use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "peer")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub room: i32,
    pub peer: String,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(belongs_to = "super::room::Entity", from = "Column::Room", to = "super::room::Column::Id")]
    Room,
    #[sea_orm(has_many = "super::peer_session::Entity")]
    Sessions,
}

impl Related<super::room::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Room.def()
    }
}

impl Related<super::peer_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Sessions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
