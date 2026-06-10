use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_cctx_status_status")
                    .table(CctxStatus::Table)
                    .col(CctxStatus::Status)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_cctx_status_status_last_update_timestamp")
                    .table(CctxStatus::Table)
                    .col(CctxStatus::Status)
                    .col(CctxStatus::LastUpdateTimestamp)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_cctx_status_status")
                    .table(CctxStatus::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_cctx_status_status_last_update_timestamp")
                    .table(CctxStatus::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum CctxStatus {
    Table,
    Status,
    LastUpdateTimestamp,
}
