use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Operation::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Operation::OperationId)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Operation::Timestamp)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Operation::NextRetry).big_integer())
                    .col(ColumnDef::new(Operation::Status).small_unsigned().not_null())
                    .to_owned(),
            )
            .await?;

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
                    .col(ColumnDef::new(OperationStage::OperationId).integer().not_null())
                    .col(ColumnDef::new(OperationStage::Stage).string().not_null())
                    .col(ColumnDef::new(OperationStage::Timestamp).big_integer().not_null())
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
                    .col(ColumnDef::new(Interval::Start).big_integer().not_null())
                    .col(ColumnDef::new(Interval::End).big_integer().not_null())
                    .col(ColumnDef::new(Interval::Timestamp).big_integer().not_null())
                    .col(ColumnDef::new(Interval::Status).small_unsigned().not_null())
                    .col(ColumnDef::new(Interval::NextRetry).big_integer().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Watermark::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Watermark::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Watermark::Timestamp).big_integer().not_null())
                    .to_owned(),
            )
            .await?;
        //specify the foreign key relationship between the two tables
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_operation_status")
                    .from(OperationStage::Table, OperationStage::OperationId)
                    .to(Operation::Table, Operation::OperationId)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OperationStage::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Operation::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Interval::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Watermark::Table).to_owned())
            .await?;

        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Operation {
    Table,
    OperationId,
    Timestamp,
    NextRetry,
    Status,
}

#[derive(Iden)]
enum OperationStage {
    Id,
    Table,
    OperationId,
    Stage,
    Timestamp,
}

#[derive(Iden)]
enum Interval {
    Id,
    Table,
    Start,
    End,
    Timestamp,
    Status,
    NextRetry,
}

#[derive(Iden)]
enum Watermark {
    Id,
    Table,
    Timestamp,
}
