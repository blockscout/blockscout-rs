use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add index for the outbound_params window function
        // This will help with the ROW_NUMBER() OVER (PARTITION BY op.cross_chain_tx_id ORDER BY op.id ASC)
        manager
            .create_index(
                Index::create()
                    .name("idx_outbound_params_cross_chain_tx_id_id")
                    .table(OutboundParams::Table)
                    .col(OutboundParams::CrossChainTxId)
                    .col(OutboundParams::Id)
                    .to_owned(),
            )
            .await?;

        // Add index for token symbol lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_token_symbol")
                    .table(Token::Table)
                    .col(Token::Symbol)
                    .to_owned(),
            )
            .await?;

        // Add composite index for token asset and symbol
        manager
            .create_index(
                Index::create()
                    .name("idx_token_asset_symbol")
                    .table(Token::Table)
                    .col(Token::Asset)
                    .col(Token::Symbol)
                    .to_owned(),
            )
            .await?;

        // Add composite index for gas tokens (coin_type + foreign_chain_id)
        manager
            .create_index(
                Index::create()
                    .name("idx_token_coin_type_foreign_chain_id")
                    .table(Token::Table)
                    .col(Token::CoinType)
                    .col(Token::ForeignChainId)
                    .to_owned(),
            )
            .await?;

        // Add index on cross_chain_tx.id for the final join
        // This should already exist as primary key, but let's make sure
        manager
            .create_index(
                Index::create()
                    .name("idx_cross_chain_tx_id_processing")
                    .table(CrossChainTx::Table)
                    .col(CrossChainTx::Id)
                    .col(CrossChainTx::ProcessingStatus)
                    .to_owned(),
            )
            .await?;

        // Add index on cctx_status for the final join
        manager
            .create_index(
                Index::create()
                    .name("idx_cctx_status_cross_chain_tx_id_status")
                    .table(CctxStatus::Table)
                    .col(CctxStatus::CrossChainTxId)
                    .col(CctxStatus::Status)
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
                    .name("idx_outbound_params_cross_chain_tx_id_id")
                    .table(OutboundParams::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_token_symbol")
                    .table(Token::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_token_asset_symbol")
                    .table(Token::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_token_coin_type_foreign_chain_id")
                    .table(Token::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_cross_chain_tx_id_processing")
                    .table(CrossChainTx::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_cctx_status_cross_chain_tx_id_status")
                    .table(CctxStatus::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum OutboundParams {
    Table,
    CrossChainTxId,
    Id,
}

#[derive(Iden)]
enum Token {
    Table,
    Symbol,
    Asset,
    CoinType,
    ForeignChainId,
}

#[derive(Iden)]
enum CrossChainTx {
    Table,
    Id,
    ProcessingStatus,
}

#[derive(Iden)]
enum CctxStatus {
    Table,
    CrossChainTxId,
    Status,
} 