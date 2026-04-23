use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add fields to operation table
        manager
            .alter_table(
                Table::alter()
                    .table(Operation::Table)
                    .add_column(ColumnDef::new(Operation::SenderAddress).string().null())
                    .add_column(ColumnDef::new(Operation::SenderBlockchain).string().null())
                    .to_owned(),
            )
            .await?;

        // Create index on operation.sender_address
        manager
            .create_index(
                Index::create()
                    .name("idx_operation_sender_address")
                    .table(Operation::Table)
                    .col(Operation::SenderAddress)
                    .to_owned(),
            )
            .await?;

        // Create operation_meta_info table
        manager
            .create_table(
                Table::create()
                    .table(OperationMetaInfo::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OperationMetaInfo::OperationId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OperationMetaInfo::TacValidExecutors)
                            .array(ColumnType::Text)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(OperationMetaInfo::TonValidExecutors)
                            .array(ColumnType::Text)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(OperationMetaInfo::TacProtocolFee)
                            .decimal()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(OperationMetaInfo::TacExecutorFee)
                            .decimal()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(OperationMetaInfo::TacTokenFeeSymbol)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(OperationMetaInfo::TonProtocolFee)
                            .decimal()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(OperationMetaInfo::TonExecutorFee)
                            .decimal()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(OperationMetaInfo::TonTokenFeeSymbol)
                            .string()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_meta_info_operation")
                            .from(OperationMetaInfo::Table, OperationMetaInfo::OperationId)
                            .to(Operation::Table, Operation::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(OperationMetaInfo::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_operation_sender_address")
                    .table(Operation::Table)
                    .to_owned(),
            )
            .await?;

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
    Id,
    SenderAddress,
    SenderBlockchain,
}

#[derive(Iden)]
enum OperationMetaInfo {
    Table,
    OperationId,
    TacValidExecutors,
    TonValidExecutors,
    TacProtocolFee,
    TacExecutorFee,
    TacTokenFeeSymbol,
    TonProtocolFee,
    TonExecutorFee,
    TonTokenFeeSymbol,
}
