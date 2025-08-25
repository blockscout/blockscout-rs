use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add composite index for the slow query filter condition
        // This will help with the WHERE clause that filters by processing_status and last_status_update_timestamp
        manager
            .create_index(
                Index::create()
                    .name("idx_cross_chain_tx_processing_status_timestamp")
                    .table(CrossChainTx::Table)
                    .col(CrossChainTx::ProcessingStatus)
                    .col(CrossChainTx::LastStatusUpdateTimestamp)
                    .to_owned(),
            )
            .await?;

        // Add index on retries_number for the exponential backoff calculation
        manager
            .create_index(
                Index::create()
                    .name("idx_cross_chain_tx_retries_number")
                    .table(CrossChainTx::Table)
                    .col(CrossChainTx::RetriesNumber)
                    .to_owned(),
            )
            .await?;

        // Add composite index for the exact filter condition used in the slow query
        // This covers: processing_status = 'Unlocked' AND last_status_update_timestamp + interval calculation < NOW()
        manager
            .create_index(
                Index::create()
                    .name("idx_cross_chain_tx_unlocked_timestamp_retries")
                    .table(CrossChainTx::Table)
                    .col(CrossChainTx::ProcessingStatus)
                    .col(CrossChainTx::LastStatusUpdateTimestamp)
                    .col(CrossChainTx::RetriesNumber)
                    .to_owned(),
            )
            .await?;



        // Add index on cctx_status.created_timestamp for the ORDER BY clause
        manager
            .create_index(
                Index::create()
                    .name("idx_cctx_status_created_timestamp")
                    .table(CctxStatus::Table)
                    .col(CctxStatus::CreatedTimestamp)
                    .to_owned(),
            )
            .await?;

        // Add composite index for the join condition and ordering
        manager
            .create_index(
                Index::create()
                    .name("idx_cctx_status_cross_chain_tx_id_created_timestamp")
                    .table(CctxStatus::Table)
                    .col(CctxStatus::CrossChainTxId)
                    .col(CctxStatus::CreatedTimestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the performance indices
        manager
            .drop_index(
                Index::drop()
                    .name("idx_cross_chain_tx_processing_status_timestamp")
                    .table(CrossChainTx::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_cross_chain_tx_retries_number")
                    .table(CrossChainTx::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_cross_chain_tx_unlocked_timestamp_retries")
                    .table(CrossChainTx::Table)
                    .to_owned(),
            )
            .await?;



        manager
            .drop_index(
                Index::drop()
                    .name("idx_cctx_status_created_timestamp")
                    .table(CctxStatus::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_cctx_status_cross_chain_tx_id_created_timestamp")
                    .table(CctxStatus::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum CrossChainTx {
    Table,
    ProcessingStatus,
    LastStatusUpdateTimestamp,
    RetriesNumber,
}

#[derive(Iden)]
enum CctxStatus {
    Table,
    CrossChainTxId,
    CreatedTimestamp,
} 