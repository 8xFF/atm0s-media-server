use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // because sqlite error with modify_column
        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .alter_table(Table::alter().table(Event::Table).modify_column(ColumnDef::new(Event::Node).big_integer()).to_owned())
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // because sqlite error with modify_column
        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .alter_table(Table::alter().table(Event::Table).modify_column(ColumnDef::new(Event::Node).unsigned()).to_owned())
                .await?;
        }
        Ok(())
    }
}

#[derive(Iden)]
enum Event {
    Table,
    Node,
}
