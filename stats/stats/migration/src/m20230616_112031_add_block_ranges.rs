use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BlockRanges::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(BlockRanges::Date).date().primary_key())
                    .col(
                        ColumnDef::new(BlockRanges::FromNumber)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlockRanges::ToNumber)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BlockRanges::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum BlockRanges {
    Table,
    Date,
    FromNumber,
    ToNumber,
}
