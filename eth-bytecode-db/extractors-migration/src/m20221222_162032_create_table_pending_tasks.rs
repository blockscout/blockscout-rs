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
                    .table(PendingTasks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PendingTasks::ChainId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PendingTasks::Address).binary().not_null())
                    .col(
                        ColumnDef::new(PendingTasks::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(SimpleExpr::Custom("CURRENT_TIMESTAMP".into())),
                    )
                    .col(
                        ColumnDef::new(PendingTasks::SourceData)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PendingTasks::Bytecode).binary().not_null())
                    .col(ColumnDef::new(PendingTasks::BytecodeType).text().not_null())
                    .col(
                        ColumnDef::new(PendingTasks::Submitted)
                            .boolean()
                            .not_null()
                            .default(SimpleExpr::Constant(Value::Bool(Some(false)))),
                    )
                    .primary_key(
                        Index::create()
                            .col(PendingTasks::ChainId)
                            .col(PendingTasks::Address),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from_col(PendingTasks::BytecodeType)
                            .to(BytecodeTypes::Table, BytecodeTypes::BytecodeType),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PendingTasks::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum PendingTasks {
    Table,
    ChainId,
    Address,
    CreatedAt,
    SourceData,
    Bytecode,
    BytecodeType,
    Submitted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_table() {
        let table = Table::create()
            .table(PendingTasks::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(PendingTasks::ChainId)
                    .big_integer()
                    .not_null(),
            )
            .col(ColumnDef::new(PendingTasks::Address).binary().not_null())
            .col(
                ColumnDef::new(PendingTasks::CreatedAt)
                    .timestamp()
                    .not_null()
                    .default(SimpleExpr::Custom("CURRENT_TIMESTAMP".into())),
            )
            .col(
                ColumnDef::new(PendingTasks::SourceData)
                    .json_binary()
                    .not_null(),
            )
            .col(ColumnDef::new(PendingTasks::Bytecode).binary().not_null())
            .col(ColumnDef::new(PendingTasks::BytecodeType).text().not_null())
            .col(
                ColumnDef::new(PendingTasks::Submitted)
                    .boolean()
                    .not_null()
                    .default(SimpleExpr::Constant(Value::Bool(Some(false)))),
            )
            .primary_key(
                Index::create()
                    .col(PendingTasks::ChainId)
                    .col(PendingTasks::Address),
            )
            .foreign_key(
                ForeignKey::create()
                    .from_col(PendingTasks::BytecodeType)
                    .to(BytecodeTypes::Table, BytecodeTypes::BytecodeType),
            )
            .to_string(PostgresQueryBuilder);
        println!("{}", table);
    }
}
