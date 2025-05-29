use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {

        // Create a composite unique index for outbound_params
        // This ensures each combination of (cross_chain_tx_id, receiver, receiver_chain_id) is unique
        manager
            .create_index(
                Index::create()
                    .name("idx_outbound_params_composite_unique")
                    .table(OutboundParams::Table)
                    .col(OutboundParams::CrossChainTxId)
                    .col(OutboundParams::Receiver)
                    .col(OutboundParams::ReceiverChainId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Create index on hash for fast lookups (but not unique since it can be null/empty)
        manager
            .create_index(
                Index::create()
                    .name("idx_outbound_params_hash")
                    .table(OutboundParams::Table)
                    .col(OutboundParams::Hash)
                    .to_owned(),
            )
            .await?;

        // Add foreign key indexes for better performance
        manager
            .create_index(
                Index::create()
                    .name("idx_cctx_status_cross_chain_tx_id")
                    .table(CctxStatus::Table)
                    .col(CctxStatus::CrossChainTxId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_inbound_params_cross_chain_tx_id")
                    .table(InboundParams::Table)
                    .col(InboundParams::CrossChainTxId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_outbound_params_cross_chain_tx_id")
                    .table(OutboundParams::Table)
                    .col(OutboundParams::CrossChainTxId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_revert_options_cross_chain_tx_id")
                    .table(RevertOptions::Table)
                    .col(RevertOptions::CrossChainTxId)
                    .to_owned(),
            )
            .await?;

        // Add index on cross_chain_tx index field for faster lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_cross_chain_tx_index")
                    .table(CrossChainTx::Table)
                    .col(CrossChainTx::Index)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove the composite unique index
        manager
            .drop_index(
                Index::drop()
                    .name("idx_outbound_params_composite_unique")
                    .table(OutboundParams::Table)
                    .to_owned(),
            )
            .await?;


        manager
            .drop_index(
                Index::drop()
                    .name("idx_outbound_params_hash")
                    .table(OutboundParams::Table)
                    .to_owned(),
            )
            .await?;
        // Remove foreign key indexes
        manager
            .drop_index(
                Index::drop()
                    .name("idx_cctx_status_cross_chain_tx_id")
                    .table(CctxStatus::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_inbound_params_cross_chain_tx_id")
                    .table(InboundParams::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_outbound_params_cross_chain_tx_id")
                    .table(OutboundParams::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_revert_options_cross_chain_tx_id")
                    .table(RevertOptions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_cross_chain_tx_index")
                    .table(CrossChainTx::Table)
                    .to_owned(),
            )
            .await?;


        Ok(())
    }
}

#[derive(Iden)]
enum CctxStatus {
    Table,
    CrossChainTxId,
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
    Receiver,
    ReceiverChainId,
    Hash,
}

#[derive(Iden)]
enum RevertOptions {
    Table,
    CrossChainTxId,
}

#[derive(Iden)]
enum CrossChainTx {
    Table,
    Index,
} 