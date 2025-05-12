use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Operation::Table)
                    .add_column(
                        ColumnDef::new(Operation::SenderAddress)
                            .string()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(Operation::SenderBlockchain)
                            .string()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Operation::Table)
                    .drop_column(Operation::SenderAddress)
                    .drop_column(Operation::SenderBlockchain)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Operation {
    Table,
    SenderAddress,
    SenderBlockchain,
}
