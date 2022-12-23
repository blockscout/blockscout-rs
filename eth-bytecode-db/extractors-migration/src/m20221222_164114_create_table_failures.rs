use super::m20221222_155714_create_table_bytecode_types::BytecodeTypes;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Failures::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Failures::ChainId).big_integer().not_null())
                    .col(ColumnDef::new(Failures::Address).binary().not_null())
                    .col(
                        ColumnDef::new(Failures::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(SimpleExpr::Custom("CURRENT_TIMESTAMP".into())),
                    )
                    .col(
                        ColumnDef::new(Failures::SourceData)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Failures::Bytecode).binary().not_null())
                    .col(ColumnDef::new(Failures::BytecodeType).text().not_null())
                    .col(ColumnDef::new(Failures::Error).text())
                    .primary_key(
                        Index::create()
                            .col(Failures::ChainId)
                            .col(Failures::Address),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from_col(Failures::BytecodeType)
                            .to(BytecodeTypes::Table, BytecodeTypes::BytecodeType),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Failures::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Failures {
    Table,
    ChainId,
    Address,
    CreatedAt,
    SourceData,
    Bytecode,
    BytecodeType,
    Error,
}
