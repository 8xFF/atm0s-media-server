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
            .alter_table(Table::alter().table(Room::Table).add_column(ColumnDef::new(Room::LastPeerLeavedAt).big_integer()).to_owned())
            .await?;
        manager
            .alter_table(Table::alter().table(PeerSession::Table).add_column(ColumnDef::new(PeerSession::Room).big_integer()).to_owned())
            .await?;
        let db = manager.get_connection();
        db.execute_unprepared(
            "UPDATE peer_session
            SET room = t1.room
            FROM peer_session t2
            INNER JOIN peer t1 on t1.id = t2.peer
            ",
        )
        .await?;

        manager
            .create_index(Index::create().name("room_last_peer_leaved_at").table(Room::Table).col(Room::LastPeerLeavedAt).to_owned())
            .await?;
        manager
            .create_index(Index::create().name("room_destroyed_at").table(Room::Table).col(Room::DestroyedAt).to_owned())
            .await?;
        manager
            .create_index(Index::create().name("peer_session_leaved_at").table(PeerSession::Table).col(PeerSession::LeavedAt).to_owned())
            .await?;
        manager
            .create_index(Index::create().name("peer_session_room").table(PeerSession::Table).col(PeerSession::Room).to_owned())
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.alter_table(Table::alter().table(Room::Table).drop_column(Room::DestroyedAt).to_owned()).await?;
        manager.alter_table(Table::alter().table(Room::Table).drop_column(Room::LastPeerLeavedAt).to_owned()).await?;
        manager.alter_table(Table::alter().table(PeerSession::Table).drop_column(PeerSession::Room).to_owned()).await?;
        manager.drop_index(Index::drop().name("room_last_peer_leaved_at").table(Room::Table).to_owned()).await?;
        manager.drop_index(Index::drop().name("room_destroyed_at").table(Room::Table).to_owned()).await?;
        manager.drop_index(Index::drop().name("peer_session_leaved_at").table(PeerSession::Table).to_owned()).await?;
        manager.drop_index(Index::drop().name("peer_session_room").table(PeerSession::Table).to_owned()).await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Room {
    Table,
    LastPeerLeavedAt,
    DestroyedAt,
}

#[derive(Iden)]
enum PeerSession {
    Table,
    Room,
    LeavedAt,
}
