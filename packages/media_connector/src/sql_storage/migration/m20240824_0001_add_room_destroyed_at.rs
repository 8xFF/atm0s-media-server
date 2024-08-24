use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(Table::alter().table(Room::Table).add_column(ColumnDef::new(Room::DestroyedAt).big_integer()).to_owned())
            .await?;
        manager
            .create_index(Index::create().name("room_destroyed_at").table(Room::Table).col(Room::DestroyedAt).to_owned())
            .await?;
        manager
            .create_index(Index::create().name("peer_session_leaved_at").table(PeerSession::Table).col(PeerSession::LeavedAt).to_owned())
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.alter_table(Table::alter().table(Room::Table).drop_column(Room::DestroyedAt).to_owned()).await?;
        manager.drop_index(Index::drop().name("room_destroyed_at").table(Room::Table).to_owned()).await?;
        manager.drop_index(Index::drop().name("peer_session_leaved_at").table(PeerSession::Table).to_owned()).await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Room {
    Table,
    DestroyedAt,
}

#[derive(Iden)]
enum PeerSession {
    Table,
    LeavedAt,
}
