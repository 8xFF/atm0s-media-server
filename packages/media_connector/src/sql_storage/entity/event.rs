use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "event")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub node: u32,
    /// This is node timestamp
    pub node_ts: i64,
    pub session: i64,
    pub created_at: i64,
    pub event: String,
    pub meta: Option<Json>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(belongs_to = "super::session::Entity", from = "Column::Session", to = "super::session::Column::Id")]
    Session,
}

impl Related<super::session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Session.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
