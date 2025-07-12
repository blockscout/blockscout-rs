use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add index for InboundParams::CrossChainTxId
        manager
            .create_index(
                Index::create()
                    .name("idx_inbound_params_cross_chain_tx_id")
                    .table(InboundParams::Table)
                    .col(InboundParams::CrossChainTxId)
                    .to_owned(),
            )
            .await?;

        // Add index for OutboundParams::CrossChainTxId
        manager
            .create_index(
                Index::create()
                    .name("idx_outbound_params_cross_chain_tx_id")
                    .table(OutboundParams::Table)
                    .col(OutboundParams::CrossChainTxId)
                    .to_owned(),
            )
            .await?;

        // Add index for RevertOptions::CrossChainTxId
        manager
            .create_index(
                Index::create()
                    .name("idx_revert_options_cross_chain_tx_id")
                    .table(RevertOptions::Table)
                    .col(RevertOptions::CrossChainTxId)
                    .to_owned(),
            )
            .await?;

        // Add composite index for status + created_timestamp for better list_cctxs performance
        manager
            .create_index(
                Index::create()
                    .name("idx_cctx_status_status_created_timestamp")
                    .table(CctxStatus::Table)
                    .col(CctxStatus::Status)
                    .col(CctxStatus::CreatedTimestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop index for InboundParams::CrossChainTxId
        manager
            .drop_index(
                Index::drop()
                    .name("idx_inbound_params_cross_chain_tx_id")
                    .table(InboundParams::Table)
                    .to_owned(),
            )
            .await?;

        // Drop index for OutboundParams::CrossChainTxId
        manager
            .drop_index(
                Index::drop()
                    .name("idx_outbound_params_cross_chain_tx_id")
                    .table(OutboundParams::Table)
                    .to_owned(),
            )
            .await?;

        // Drop index for RevertOptions::CrossChainTxId
        manager
            .drop_index(
                Index::drop()
                    .name("idx_revert_options_cross_chain_tx_id")
                    .table(RevertOptions::Table)
                    .to_owned(),
            )
            .await?;

        // Drop composite index for status + created_timestamp
        manager
            .drop_index(
                Index::drop()
                    .name("idx_cctx_status_status_created_timestamp")
                    .table(CctxStatus::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum InboundParams {
    Table,
    CrossChainTxId,
}

#[derive(Iden)]
enum OutboundParams {
    Table,
    CrossChainTxId,
}

#[derive(Iden)]
enum RevertOptions {
    Table,
    CrossChainTxId,
}

#[derive(Iden)]
enum CctxStatus {
    Table,
    Status,
    CreatedTimestamp,
} 