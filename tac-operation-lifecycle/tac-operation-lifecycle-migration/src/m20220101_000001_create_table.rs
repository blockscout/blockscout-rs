use sea_orm::{EnumIter, Iterable};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
struct StatusEnum;

#[derive(DeriveIden, EnumIter)]
enum StatusVariants {
    #[sea_orm(iden = "pending")]
    Pending,
    #[sea_orm(iden = "processing")]
    Processing,
    #[sea_orm(iden = "completed")]
    Completed,
    #[sea_orm(iden = "failed")]
    Failed,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Creating status type for `interval` and `operation` tables
        manager
            .create_type(
                extension::postgres::Type::create()
                    .as_enum(StatusEnum)
                    .values(StatusVariants::iter())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("watermark"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WaterMark::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(WaterMark::Timestamp).timestamp().not_null())
                    .to_owned(),
            )
            .await?;
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
                    .col(ColumnDef::new(Operation::OpType).string().null())
                    .col(ColumnDef::new(Operation::Timestamp).timestamp().not_null())
                    .col(ColumnDef::new(Operation::NextRetry).timestamp().null())
                    .col(
                        ColumnDef::new(Interval::Status)
                            .custom(Alias::new("status_enum"))
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Operation::RetryCount)
                            .small_unsigned()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Operation::InsertedAt).timestamp().not_null())
                    .col(ColumnDef::new(Operation::UpdatedAt).timestamp().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_operation_status_timestamp")
                    .table(Operation::Table)
                    .col(Operation::Status)
                    .col(Operation::Timestamp)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_operation_id")
                    .table(Operation::Table)
                    .col(Operation::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_operation_status_next_retry_timestamp")
                    .table(Operation::Table)
                    .col(Operation::Status)
                    .col(Operation::NextRetry)
                    .col(Operation::Timestamp)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Interval::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Interval::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Interval::Start).timestamp().not_null())
                    .col(ColumnDef::new(Interval::Finish).timestamp().not_null())
                    .col(ColumnDef::new(Interval::InsertedAt).timestamp().not_null())
                    .col(ColumnDef::new(Interval::UpdatedAt).timestamp().not_null())
                    .col(
                        ColumnDef::new(Interval::Status)
                            .custom(Alias::new("status_enum"))
                            .not_null(),
                    )
                    .col(ColumnDef::new(Interval::NextRetry).timestamp().null())
                    .col(
                        ColumnDef::new(Interval::RetryCount)
                            .small_unsigned()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_interval_status_start")
                    .table(Interval::Table)
                    .col(Interval::Status)
                    .col(Interval::Start)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_interval_id")
                    .table(Interval::Table)
                    .col(Interval::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_interval_id_status")
                    .table(Interval::Table)
                    .col(Interval::Id)
                    .col(Interval::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_interval_status_end")
                    .table(Interval::Table)
                    .col(Interval::Status)
                    .col(Interval::Finish)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_interval_status_next_retry")
                    .table(Interval::Table)
                    .col(Interval::Status)
                    .col(Interval::NextRetry)
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
                            .small_unsigned()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OperationStage::Success).boolean().not_null())
                    .col(
                        ColumnDef::new(OperationStage::Timestamp)
                            .timestamp()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OperationStage::Note).string().null())
                    .col(
                        ColumnDef::new(OperationStage::InsertedAt)
                            .timestamp()
                            .not_null(),
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
                    .col(ColumnDef::new(Transaction::StageId).integer().not_null())
                    .col(ColumnDef::new(Transaction::Hash).string().not_null())
                    .col(
                        ColumnDef::new(Transaction::InsertedAt)
                            .timestamp()
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
        manager
            .drop_table(Table::drop().table(Alias::new("watermark")).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Interval::Table).to_owned())
            .await?;
        manager
            .drop_type(
                extension::postgres::Type::drop()
                    .if_exists()
                    .name(Alias::new("status_enum"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Operation {
    Table,
    Id,
    OpType,
    Timestamp,
    NextRetry,
    Status,
    RetryCount,
    InsertedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum WaterMark {
    Id,
    Timestamp,
}
#[derive(Iden)]
enum Interval {
    Table,
    Id,
    Start,
    Finish,
    Status,
    NextRetry,
    RetryCount,
    InsertedAt,
    UpdatedAt,
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
    InsertedAt,
}

#[derive(Iden)]
enum Transaction {
    Table,
    Id,
    StageId,
    Hash,
    BlockchainType,
    InsertedAt,
}
