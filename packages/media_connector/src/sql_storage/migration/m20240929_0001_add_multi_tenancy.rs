use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(Table::alter().table(Room::Table).add_column(ColumnDef::new(Room::App).string().default("")).to_owned())
            .await?;
        manager.create_index(Index::create().name("room_app").table(Room::Table).col(Room::App).to_owned()).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("room_app").table(Room::Table).to_owned()).await?;
        manager.alter_table(Table::alter().table(Room::Table).drop_column(Room::App).to_owned()).await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Room {
    Table,
    App,
}
