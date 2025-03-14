use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create Operation table
        manager
            .create_table(
                Table::create()
                    .table(Operation::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Operation::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Operation::OperationType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Operation::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create StageType enum table
        manager
            .create_table(
                Table::create()
                    .table(StageType::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(StageType::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(StageType::Name)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create OperationStage table
        manager
            .create_table(
                Table::create()
                    .table(OperationStage::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OperationStage::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OperationStage::OperationId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OperationStage::StageTypeId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OperationStage::Success)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OperationStage::Timestamp)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OperationStage::Note)
                            .string()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create Transaction table
        manager
            .create_table(
                Table::create()
                    .table(Transaction::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Transaction::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Transaction::StageId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Transaction::Hash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Transaction::BlockchainType)
                            .string()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add foreign key constraints
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_stage_operation")
                    .from(OperationStage::Table, OperationStage::OperationId)
                    .to(Operation::Table, Operation::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_stage_type")
                    .from(OperationStage::Table, OperationStage::StageTypeId)
                    .to(StageType::Table, StageType::Id)
                    .on_delete(ForeignKeyAction::Restrict)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_transaction_stage")
                    .from(Transaction::Table, Transaction::StageId)
                    .to(OperationStage::Table, OperationStage::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Transaction::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(OperationStage::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(StageType::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Operation::Table).to_owned())
            .await?;

        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Operation {
    Table,
    Id,
    OperationType,
    CreatedAt,
}

#[derive(Iden)]
enum StageType {
    Table,
    Id,
    Name,
}

#[derive(Iden)]
enum OperationStage {
    Table,
    Id,
    OperationId,
    StageTypeId,
    Success,
    Timestamp,
    Note,
}

#[derive(Iden)]
enum Transaction {
    Table,
    Id,
    StageId,
    Hash,
    BlockchainType,
}
