use sea_orm_migration::{MigrationTrait, MigratorTrait};

mod m20240626_0001_init;
mod m20240809_0001_change_node_id_i64;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20240626_0001_init::Migration), Box::new(m20240809_0001_change_node_id_i64::Migration)]
    }
}
