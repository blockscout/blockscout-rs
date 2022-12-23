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
                    .table(VerifiedContracts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(VerifiedContracts::ChainId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VerifiedContracts::Address)
                            .binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VerifiedContracts::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(SimpleExpr::Custom("CURRENT_TIMESTAMP".into())),
                    )
                    .col(
                        ColumnDef::new(VerifiedContracts::SourceData)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VerifiedContracts::Bytecode)
                            .binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VerifiedContracts::BytecodeType)
                            .text()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(VerifiedContracts::ChainId)
                            .col(VerifiedContracts::Address),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from_col(VerifiedContracts::BytecodeType)
                            .to(BytecodeTypes::Table, BytecodeTypes::BytecodeType),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(VerifiedContracts::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum VerifiedContracts {
    Table,
    ChainId,
    Address,
    CreatedAt,
    SourceData,
    Bytecode,
    BytecodeType,
}
