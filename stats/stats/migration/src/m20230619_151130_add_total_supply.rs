use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NativeCoinSupplyData::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NativeCoinSupplyData::Address)
                            .binary()
                            .primary_key()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeCoinSupplyData::Balance)
                            .decimal_len(100, 0)
                            .not_null(),
                    )
                    .col(ColumnDef::new(NativeCoinSupplyData::Date).date().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NativeCoinSupplyData::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum NativeCoinSupplyData {
    Table,
    Address,
    Balance,
    Date,
}
