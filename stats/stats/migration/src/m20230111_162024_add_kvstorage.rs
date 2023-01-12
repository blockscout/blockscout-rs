use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(KVStorage::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(KVStorage::Key)
                            .char_len(255)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(KVStorage::Value).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(KVStorage::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum KVStorage {
    Table,
    Key,
    Value,
}
