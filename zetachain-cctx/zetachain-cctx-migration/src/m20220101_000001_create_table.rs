use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create the enum type first
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"DO $$ BEGIN
                IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'tx_finalization_status') THEN
                    CREATE TYPE tx_finalization_status AS ENUM ('NotFinalized', 'Finalized', 'Executed');
                END IF;
                IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'kind') THEN
                    CREATE TYPE kind AS ENUM ('realtime', 'historical');
                END IF;
                IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'cctx_status_status') THEN
                    CREATE TYPE cctx_status_status AS ENUM ('PendingInbound', 'PendingOutbound', 'PendingRevert', 'Aborted', 'Reverted', 'OutboundMined');
                END IF;
                IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'inbound_status') THEN
                    CREATE TYPE inbound_status AS ENUM ('SUCCESS', 'INSUFFICIENT_DEPOSITOR_FEE', 'INVALID_RECEIVER_ADDRESS', 'INVALID_MEMO');
                END IF;
                IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'confirmation_mode') THEN
                    CREATE TYPE confirmation_mode AS ENUM ('SAFE', 'FAST');
                END IF;
                IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'coin_type') THEN
                    CREATE TYPE coin_type AS ENUM ('Zeta', 'Gas', 'Erc20', 'Cmd', 'NoAssetCall');
                END IF;
                IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'protocol_contract_version') THEN
                    CREATE TYPE protocol_contract_version AS ENUM ('V1', 'V2');
                END IF;
                IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'processing_status') THEN
                    CREATE TYPE processing_status AS ENUM ('locked', 'unlocked', 'failed', 'done');
                END IF;
            END $$;"#,
        )
        .await?;

        manager
            .create_table(
                Table::create()
                    .table(Watermark::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Watermark::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Watermark::Kind)
                            .enumeration("kind", ["realtime", "historical"])
                            .not_null(),
                    )
                    .col(ColumnDef::new(Watermark::Pointer).string().not_null())
                    .col(
                        ColumnDef::new(Watermark::ProcessingStatus)
                            .enumeration("processing_status", ["locked", "unlocked", "failed", "done"])
                            .not_null()
                            .default("unlocked"),
                    )
                    .col(
                        ColumnDef::new(Watermark::CreatedAt)
                            .date_time()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Watermark::UpdatedAt)
                            .date_time()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Watermark::UpdatedBy)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Watermark::RetriesNumber)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),    
            )
            .await?;
        // Create cross_chain_txs table
        manager
            .create_table(
                Table::create()
                    .table(CrossChainTx::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrossChainTx::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CrossChainTx::Creator).string().not_null())
                    .col(ColumnDef::new(CrossChainTx::Index).string().not_null())
                    .col(ColumnDef::new(CrossChainTx::ZetaFees).string().not_null())
                    .col(ColumnDef::new(CrossChainTx::RetriesNumber).integer().not_null().default(0))
                    .col(
                        ColumnDef::new(CrossChainTx::ProcessingStatus)
                            .enumeration("processing_status", ["locked", "unlocked", "failed", "done"])
                            .not_null()
                            .default("unlocked"),
                    )
                    .col(ColumnDef::new(CrossChainTx::RelayedMessage).text().null())
                    .col(
                        ColumnDef::new(CrossChainTx::LastStatusUpdateTimestamp)
                            .date_time()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CrossChainTx::ProtocolContractVersion)
                            .enumeration("protocol_contract_version", ["V1", "V2"])
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CrossChainTx::RootId)
                            .integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(CrossChainTx::ParentId)
                            .integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(CrossChainTx::Depth)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(CrossChainTx::UpdatedBy).string().not_null())
                    .to_owned(),
            )
            .await?;

        // Add foreign key constraints for self-referential relationships
        
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_cross_chain_tx_root")
                    .from(CrossChainTx::Table, CrossChainTx::RootId)
                    .to(CrossChainTx::Table, CrossChainTx::Id)
                    .on_delete(ForeignKeyAction::SetNull)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_cross_chain_tx_parent")
                    .from(CrossChainTx::Table, CrossChainTx::ParentId)
                    .to(CrossChainTx::Table, CrossChainTx::Id)
                    .on_delete(ForeignKeyAction::SetNull)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cross_chain_tx_index_unique")
                    .table(CrossChainTx::Table)
                    .col(CrossChainTx::Index)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Create cctx_statuses table
        manager
            .create_table(
                Table::create()
                    .table(CctxStatus::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CctxStatus::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CctxStatus::CrossChainTxId)
                            .integer()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(CctxStatus::Status)
                            .enumeration(
                                "cctx_status_status",
                                [
                                    "PendingInbound",
                                    "PendingOutbound",
                                    "PendingRevert",
                                    "Aborted",
                                    "Reverted",
                                    "OutboundMined",
                                ],
                            )
                            .not_null(),
                    )
                    .col(ColumnDef::new(CctxStatus::StatusMessage).string().null())
                    .col(ColumnDef::new(CctxStatus::ErrorMessage).text().null())
                    .col(
                        ColumnDef::new(CctxStatus::LastUpdateTimestamp)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CctxStatus::IsAbortRefunded)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CctxStatus::CreatedTimestamp)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CctxStatus::ErrorMessageRevert).text().null())
                    .col(ColumnDef::new(CctxStatus::ErrorMessageAbort).text().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cctx_status_cross_chain_tx")
                            .from(CctxStatus::Table, CctxStatus::CrossChainTxId)
                            .to(CrossChainTx::Table, CrossChainTx::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create inbound_params table
        manager
            .create_table(
                Table::create()
                    .table(InboundParams::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InboundParams::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(InboundParams::CrossChainTxId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(InboundParams::Sender).string().not_null())
                    .col(
                        ColumnDef::new(InboundParams::SenderChainId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(InboundParams::TxOrigin).string().not_null())
                    .col(ColumnDef::new(InboundParams::CoinType).enumeration("coin_type", ["Zeta", "Gas", "Erc20", "Cmd", "NoAssetCall"]).not_null())
                    .col(ColumnDef::new(InboundParams::Asset).string().null())
                    .col(ColumnDef::new(InboundParams::Amount).string().not_null())
                    .col(
                        ColumnDef::new(InboundParams::ObservedHash)
                            .string()
                            .unique_key()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InboundParams::ObservedExternalHeight)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InboundParams::BallotIndex)
                            .string()
                            .unique_key()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InboundParams::FinalizedZetaHeight)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InboundParams::TxFinalizationStatus)
                            .enumeration(
                                "tx_finalization_status",
                                ["NotFinalized", "Finalized", "Executed"],
                            )
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InboundParams::IsCrossChainCall)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InboundParams::Status)
                            .enumeration("inbound_status", ["SUCCESS", "INSUFFICIENT_DEPOSITOR_FEE", "INVALID_RECEIVER_ADDRESS", "INVALID_MEMO"])
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InboundParams::ConfirmationMode)
                            .enumeration("confirmation_mode", ["SAFE", "FAST"])
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_inbound_params_cross_chain_tx")
                            .from(InboundParams::Table, InboundParams::CrossChainTxId)
                            .to(CrossChainTx::Table, CrossChainTx::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create outbound_params table
        manager
            .create_table(
                Table::create()
                    .table(OutboundParams::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OutboundParams::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OutboundParams::CrossChainTxId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OutboundParams::Receiver).string().not_null())
                    .col(
                        ColumnDef::new(OutboundParams::ReceiverChainId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OutboundParams::CoinType).enumeration("coin_type", ["Zeta", "Gas", "Erc20", "Cmd", "NoAssetCall"]).not_null())
                    .col(ColumnDef::new(OutboundParams::Amount).string().not_null())
                    .col(ColumnDef::new(OutboundParams::TssNonce).string().not_null())
                    .col(ColumnDef::new(OutboundParams::GasLimit).string().not_null())
                    .col(ColumnDef::new(OutboundParams::GasPrice).string().null())
                    .col(
                        ColumnDef::new(OutboundParams::GasPriorityFee)
                            .string()
                            .null(),
                    )
                    .col(ColumnDef::new(OutboundParams::Hash).string().unique_key().not_null())
                    .col(ColumnDef::new(OutboundParams::BallotIndex).string().null())
                    .col(
                        ColumnDef::new(OutboundParams::ObservedExternalHeight)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OutboundParams::GasUsed).string().not_null())
                    .col(
                        ColumnDef::new(OutboundParams::EffectiveGasPrice)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OutboundParams::EffectiveGasLimit)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OutboundParams::TssPubkey)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OutboundParams::TxFinalizationStatus)
                            .enumeration(
                                "tx_finalization_status",
                                ["NotFinalized", "Finalized", "Executed"],
                            )
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OutboundParams::CallOptionsGasLimit)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(OutboundParams::CallOptionsIsArbitraryCall)
                            .boolean()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(OutboundParams::ConfirmationMode)
                            .enumeration("confirmation_mode", ["SAFE", "FAST"])
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_outbound_params_cross_chain_tx")
                            .from(OutboundParams::Table, OutboundParams::CrossChainTxId)
                            .to(CrossChainTx::Table, CrossChainTx::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create revert_options table
        manager
            .create_table(
                Table::create()
                    .table(RevertOptions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RevertOptions::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RevertOptions::CrossChainTxId)
                            .integer()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(RevertOptions::RevertAddress).string().null())
                    .col(
                        ColumnDef::new(RevertOptions::CallOnRevert)
                            .boolean()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RevertOptions::AbortAddress).string().null())
                    .col(ColumnDef::new(RevertOptions::RevertMessage).text().null())
                    .col(
                        ColumnDef::new(RevertOptions::RevertGasLimit)
                            .string()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_revert_options_cross_chain_tx")
                            .from(RevertOptions::Table, RevertOptions::CrossChainTxId)
                            .to(CrossChainTx::Table, CrossChainTx::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop foreign key constraints first
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_cross_chain_tx_root")
                    .table(CrossChainTx::Table)
                    .to_owned(),
            )
            .await?;
            
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_cross_chain_tx_parent")
                    .table(CrossChainTx::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_cross_chain_tx_index_unique")
                    .table(CrossChainTx::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Watermark::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RevertOptions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(OutboundParams::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(InboundParams::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CctxStatus::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CrossChainTx::Table).to_owned())
            .await?;

        // Drop the enum type
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"DO $$ BEGIN
                IF EXISTS (SELECT 1 FROM pg_type WHERE typname = 'tx_finalization_status') THEN
                    DROP TYPE tx_finalization_status;
                END IF;
                IF EXISTS (SELECT 1 FROM pg_type WHERE typname = 'processing_status') THEN
                    DROP TYPE processing_status;
                END IF;
            END $$;"#,
        )
        .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Watermark {
    Table,
    Id,
    Kind,
    Pointer,
    ProcessingStatus,
    CreatedAt,
    UpdatedAt,
    UpdatedBy,
    RetriesNumber,
}
/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum CrossChainTx {
    Table,
    Id,
    Creator,
    Index,
    ZetaFees,
    ProcessingStatus,
    RelayedMessage,
    ProtocolContractVersion,
    LastStatusUpdateTimestamp,
    RootId,
    ParentId,
    Depth,
    RetriesNumber,
    UpdatedBy
}

#[derive(Iden)]
enum CctxStatus {
    Table,
    Id,
    CrossChainTxId,
    Status,
    StatusMessage,
    ErrorMessage,
    LastUpdateTimestamp,
    IsAbortRefunded,
    CreatedTimestamp,
    ErrorMessageRevert,
    ErrorMessageAbort,
}

#[derive(Iden)]
enum InboundParams {
    Table,
    Id,
    CrossChainTxId,
    Sender,
    SenderChainId,
    TxOrigin,
    CoinType,
    Asset,
    Amount,
    ObservedHash,
    ObservedExternalHeight,
    BallotIndex,
    FinalizedZetaHeight,
    TxFinalizationStatus,
    IsCrossChainCall,
    Status,
    ConfirmationMode,
}

#[derive(Iden)]
enum OutboundParams {
    Table,
    Id,
    CrossChainTxId,
    Receiver,
    ReceiverChainId,
    CoinType,
    Amount,
    TssNonce,
    GasLimit,
    GasPrice,
    GasPriorityFee,
    Hash,
    BallotIndex,
    ObservedExternalHeight,
    GasUsed,
    EffectiveGasPrice,
    EffectiveGasLimit,
    TssPubkey,
    TxFinalizationStatus,
    CallOptionsGasLimit,
    CallOptionsIsArbitraryCall,
    ConfirmationMode,
}

#[derive(Iden)]
enum RevertOptions {
    Table,
    Id,
    CrossChainTxId,
    RevertAddress,
    CallOnRevert,
    AbortAddress,
    RevertMessage,
    RevertGasLimit,
}
