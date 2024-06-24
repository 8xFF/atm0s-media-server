use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Room::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Room::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Room::Room).string().not_null())
                    .col(ColumnDef::new(Room::CreatedAt).big_integer().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Peer::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Peer::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Peer::Room).integer().not_null())
                    .col(ColumnDef::new(Peer::Peer).string().not_null())
                    .col(ColumnDef::new(Peer::CreatedAt).big_integer().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Session::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Session::Id).big_integer().not_null().primary_key())
                    .col(ColumnDef::new(Session::Ip).string())
                    .col(ColumnDef::new(Session::UserAgent).string())
                    .col(ColumnDef::new(Session::Sdk).string())
                    .col(ColumnDef::new(Session::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Session::JoinedAt).big_integer())
                    .col(ColumnDef::new(Session::LeavedAt).big_integer())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PeerSession::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(PeerSession::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(PeerSession::Peer).integer().not_null())
                    .col(ColumnDef::new(PeerSession::Session).big_integer().not_null())
                    .col(ColumnDef::new(PeerSession::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(PeerSession::JoinedAt).big_integer().not_null())
                    .col(ColumnDef::new(PeerSession::LeavedAt).big_integer())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Event::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Event::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Event::Node).unsigned().not_null())
                    .col(ColumnDef::new(Event::NodeTs).big_integer().not_null())
                    .col(ColumnDef::new(Event::Session).big_integer().not_null())
                    .col(ColumnDef::new(Event::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Event::Event).string().not_null())
                    .col(ColumnDef::new(Event::Meta).json())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Room::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Peer::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Session::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(PeerSession::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Event::Table).to_owned()).await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Room {
    Table,
    Id,
    Room,
    CreatedAt,
}

#[derive(Iden)]
enum Peer {
    Table,
    Id,
    Room,
    Peer,
    CreatedAt,
}

#[derive(Iden)]
enum PeerSession {
    Table,
    Id,
    Peer,
    Session,
    CreatedAt,
    JoinedAt,
    LeavedAt,
}

#[derive(Iden)]
enum Session {
    Table,
    Id,
    Ip,
    UserAgent,
    Sdk,
    CreatedAt,
    JoinedAt,
    LeavedAt,
}

#[derive(Iden)]
enum Event {
    Table,
    Id,
    Node,
    NodeTs,
    Session,
    CreatedAt,
    Event,
    Meta,
}
