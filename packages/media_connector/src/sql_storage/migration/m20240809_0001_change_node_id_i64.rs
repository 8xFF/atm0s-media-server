use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // because sqlite error with modify_column then we don't fire error here
        // TODO: don't run with sqlite
        if let Err(e) = manager
            .alter_table(Table::alter().table(Event::Table).modify_column(ColumnDef::new(Event::Node).big_integer()).to_owned())
            .await
        {
            log::error!("modify_column event.node to i64 error {e:?}");
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // because sqlite error with modify_column then we don't fire error here
        // TODO: don't run with sqlite
        if let Err(e) = manager
            .alter_table(Table::alter().table(Event::Table).modify_column(ColumnDef::new(Event::Node).unsigned()).to_owned())
            .await
        {
            log::error!("modify_column event.node to i64 error {e:?}");
        }
        Ok(())
    }
}

#[derive(Iden)]
enum Event {
    Table,
    Node,
}
