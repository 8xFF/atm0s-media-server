use sea_orm_migration::{MigrationTrait, MigratorTrait};

mod m20240626_0001_init;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20240626_0001_init::Migration)]
    }
}
