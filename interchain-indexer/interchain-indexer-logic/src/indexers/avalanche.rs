use std::{collections::HashMap, time::Duration};

use alloy::{
    network::Ethereum,
    primitives::Address,
    providers::{DynProvider, Provider as _},
    rpc::types::{Filter, Log},
    sol,
    sol_types::SolEvent,
};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use futures::{StreamExt, stream};
use interchain_indexer_entity::{
    crosschain_messages, indexer_checkpoints, sea_orm_active_enums::MessageStatus,
};
use sea_orm::{
    ActiveValue, DatabaseTransaction, DbErr, EntityTrait, TransactionTrait,
    sea_query::{Expr, OnConflict, SimpleExpr},
};
use tokio::task::JoinHandle;

use crate::{InterchainDatabase, log_stream::LogStreamBuilder};

sol! {
    struct TeleporterMessageReceipt {
        uint256 receivedMessageNonce;
        address relayerRewardAddress;
    }

    struct TeleporterFeeInfo {
        address feeTokenAddress;
        uint256 amount;
    }

    struct TeleporterMessage {
        uint256 messageNonce;
        address originSenderAddress;
        bytes32 destinationBlockchainID;
        address destinationAddress;
        uint256 requiredGasLimit;
        address[] allowedRelayerAddresses;
        TeleporterMessageReceipt[] receipts;
        bytes message;
    }
    interface ITeleporterMessenger {
        event SendCrossChainMessage(
            bytes32 indexed messageID,
            bytes32 indexed destinationBlockchainID,
            TeleporterMessage message,
            TeleporterFeeInfo feeInfo
        );

        event ReceiveCrossChainMessage(
            bytes32 indexed messageID,
            bytes32 indexed sourceBlockchainID,
            address indexed deliverer,
            address rewardRedeemer,
            TeleporterMessage message
        );

        event MessageExecuted(bytes32 indexed messageID, bytes32 indexed sourceBlockchainID);

        event MessageExecutionFailed(
            bytes32 indexed messageID, bytes32 indexed sourceBlockchainID, TeleporterMessage message
        );
    }
}

#[derive(Clone, Debug)]
pub struct AvalancheChainConfig {
    pub chain_id: i64,
    pub provider: DynProvider<Ethereum>,
    pub contract_address: Address,
    pub start_block: u64,
}

#[derive(Clone, Debug)]
pub struct AvalancheIndexerConfig {
    pub bridge_id: i32,
    pub chains: Vec<AvalancheChainConfig>,
    pub poll_interval: Duration,
    pub batch_size: u64,
}

impl AvalancheIndexerConfig {
    pub fn new(bridge_id: i32, chains: Vec<AvalancheChainConfig>) -> Self {
        Self {
            bridge_id,
            chains,
            poll_interval: Duration::from_secs(10),
            batch_size: 2000,
        }
    }

    pub fn with_poll_interval(mut self, poll_interval: Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }

    pub fn with_batch_size(mut self, batch_size: u64) -> Self {
        self.batch_size = batch_size;
        self
    }
}

struct AvalancheIndexer {
    db: InterchainDatabase,
    config: AvalancheIndexerConfig,
}

impl AvalancheIndexer {
    fn new(db: InterchainDatabase, config: AvalancheIndexerConfig) -> Result<Self> {
        if config.chains.is_empty() {
            return Err(anyhow!(
                "Avalanche indexer requires at least one configured chain"
            ));
        }

        Ok(Self { db, config })
    }

    async fn run(self) -> Result<()> {
        let AvalancheIndexer { db, config } = self;
        let AvalancheIndexerConfig {
            bridge_id,
            chains,
            poll_interval,
            batch_size,
        } = config;

        tracing::info!(
            bridge_id,
            chain_count = chains.len(),
            "Starting Avalanche indexer"
        );

        let mut combined_stream = stream::empty::<(i64, Vec<Log>)>().boxed();

        for chain in chains {
            let chain_id = chain.chain_id;
            let start_block = chain.start_block;
            let contract_address = chain.contract_address;
            let provider = chain.provider.clone();

            // Restore checkpoint if it exists for this bridge and chain.
            let checkpoint = db.get_checkpoint(bridge_id as u64, chain_id as u64).await?;

            let (forward_cursor, backward_cursor) = if let Some(cp) = checkpoint {
                let forward_cursor = cp.realtime_cursor.max(0) as u64;
                let backward_cursor = cp.catchup_max_block.max(0) as u64;

                tracing::info!(
                    bridge_id,
                    chain_id,
                    forward_cursor,
                    backward_cursor,
                    "Restored Avalanche indexer checkpoint"
                );

                (forward_cursor, backward_cursor)
            } else {
                // No checkpoint yet: start from configuration.
                let latest_block = provider.get_block_number().await.with_context(|| {
                    format!("failed to fetch latest block for chain {chain_id}")
                })?;
                (latest_block, latest_block.saturating_add(1))
            };

            let filter = Filter::new()
                .address(vec![contract_address])
                .events(ITeleporterMessenger::ITeleporterMessengerEvents::SIGNATURES);

            tracing::info!(bridge_id, chain_id, "Configured log stream");

            let stream = LogStreamBuilder::new(provider.clone())
                .filter(filter)
                .poll_interval(poll_interval)
                .batch_size(batch_size)
                .genesis_block(start_block)
                .forward_cursor(forward_cursor)
                .backward_cursor(backward_cursor)
                .catchup()
                .realtime()
                .into_stream()
                .map(move |logs| (chain_id, logs))
                .boxed();

            combined_stream = stream::select(combined_stream, stream).boxed();
        }

        let native_id_to_chain_id = db
            .load_native_id_map()
            .await
            .context("failed to preload native blockchain id mapping")?;

        while let Some((chain_id, batch)) = combined_stream.next().await {
            match process_batch(batch, chain_id, bridge_id, &native_id_to_chain_id, &db).await {
                Ok(()) => {
                    tracing::info!(bridge_id, chain_id, "Processed Avalanche log batch");
                }
                Err(err) => {
                    tracing::error!(
                        err = ?err,
                        bridge_id,
                        chain_id,
                        "Failed to process Avalanche log batch"
                    );
                }
            }
        }

        tracing::warn!(bridge_id, "Avalanche indexer stream completed unexpectedly");
        Ok(())
    }
}

pub fn spawn_indexer(
    db: InterchainDatabase,
    config: AvalancheIndexerConfig,
) -> Result<JoinHandle<()>> {
    let bridge_id = config.bridge_id;
    let indexer = AvalancheIndexer::new(db, config)?;

    let handle = tokio::spawn(async move {
        if let Err(err) = indexer.run().await {
            tracing::error!(err = ?err, bridge_id, "Avalanche indexer terminated with error");
        }
    });

    Ok(handle)
}

fn blockchain_id_hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

async fn upsert_message(
    tx: &DatabaseTransaction,
    message: crosschain_messages::ActiveModel,
    update_columns: Vec<crosschain_messages::Column>,
) -> Result<(), DbErr> {
    crosschain_messages::Entity::insert(message)
        .on_conflict(
            OnConflict::columns([
                crosschain_messages::Column::Id,
                crosschain_messages::Column::BridgeId,
            ])
            .update_columns(update_columns)
            .to_owned(),
        )
        .exec(tx)
        .await?;

    Ok(())
}

async fn process_batch(
    batch: Vec<Log>,
    chain_id: i64,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    interchain_db: &InterchainDatabase,
) -> Result<()> {
    let native_id_to_chain_id = native_id_to_chain_id.clone();
    interchain_db
        .db
        .transaction::<_, (), DbErr>(move |tx| {
            Box::pin(async move {
                for log in &batch {
                    handle_log(chain_id, log, bridge_id, tx, &native_id_to_chain_id)
                        .await
                        .map_err(|e| DbErr::Custom(e.to_string()))?;
                }

                let max_block = batch
                    .last()
                    .and_then(|log| log.block_number)
                    .ok_or_else(|| DbErr::Custom("missing block number".to_string()))?
                    as i64;

                update_cursors(bridge_id, chain_id, max_block, tx).await?;

                Ok(())
            })
        })
        .await?;

    Ok(())
}

async fn handle_log(
    chain_id: i64,
    log: &Log,
    bridge_id: i32,
    tx: &DatabaseTransaction,
    native_id_to_chain_id: &HashMap<String, i64>,
) -> anyhow::Result<()> {
    match log.topic0() {
        Some(&ITeleporterMessenger::SendCrossChainMessage::SIGNATURE_HASH) => {
            handle_send_cross_chain_message(chain_id, log, bridge_id, tx, native_id_to_chain_id)
                .await
        }
        Some(&ITeleporterMessenger::ReceiveCrossChainMessage::SIGNATURE_HASH) => {
            handle_receive_cross_chain_message(chain_id, log, bridge_id, tx, native_id_to_chain_id)
                .await
        }
        Some(&ITeleporterMessenger::MessageExecuted::SIGNATURE_HASH) => {
            handle_message_executed(chain_id, log, bridge_id, tx, native_id_to_chain_id).await
        }
        Some(&ITeleporterMessenger::MessageExecutionFailed::SIGNATURE_HASH) => {
            handle_message_execution_failed(chain_id, log, bridge_id, tx, native_id_to_chain_id)
                .await
        }
        _ => Err(anyhow::Error::msg("unknown event")),
    }
}

async fn handle_send_cross_chain_message(
    chain_id: i64,
    log: &Log,
    bridge_id: i32,
    tx: &DatabaseTransaction,
    native_id_to_chain_id: &HashMap<String, i64>,
) -> Result<()> {
    let event = log
        .log_decode::<ITeleporterMessenger::SendCrossChainMessage>()?
        .inner;
    let message_id_bytes = event.messageID.as_slice();
    let id = i64::from_be_bytes(message_id_bytes[..8].try_into()?);
    let block_number = log.block_number.context("missing block number")?;
    let tx_hash = log.transaction_hash.context("missing tx hash")?;
    let sender_bytes = event.message.originSenderAddress.as_slice().to_vec();
    let recipient_bytes = event.message.destinationAddress.as_slice().to_vec();
    let now = Utc::now().naive_utc();

    let update_columns = vec![
        crosschain_messages::Column::SrcChainId,
        crosschain_messages::Column::SrcTxHash,
        crosschain_messages::Column::SenderAddress,
        crosschain_messages::Column::RecipientAddress,
        crosschain_messages::Column::Payload,
    ];

    let destination_hex = blockchain_id_hex(event.destinationBlockchainID.as_slice());
    let dst_chain_id = native_id_to_chain_id.get(&destination_hex).copied();

    let message = crosschain_messages::ActiveModel {
        id: ActiveValue::Set(id),
        bridge_id: ActiveValue::Set(bridge_id),
        status: ActiveValue::Set(MessageStatus::Initiated),
        src_chain_id: ActiveValue::Set(chain_id),
        dst_chain_id: ActiveValue::Set(dst_chain_id),
        init_timestamp: ActiveValue::Set(Some(now)),
        last_update_timestamp: ActiveValue::Set(Some(now)),
        src_tx_hash: ActiveValue::Set(Some(tx_hash.as_slice().to_vec())),
        sender_address: ActiveValue::Set(Some(sender_bytes)),
        recipient_address: ActiveValue::Set(Some(recipient_bytes)),
        payload: ActiveValue::Set(Some(event.message.message.to_vec())),
        updated_at: ActiveValue::Set(Some(Utc::now().naive_utc())),
        ..Default::default()
    };

    upsert_message(tx, message, update_columns).await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        nonce = %event.message.messageNonce,
        "Processed SendCrossChainMessage"
    );

    Ok(())
}

async fn handle_receive_cross_chain_message(
    chain_id: i64,
    log: &Log,
    bridge_id: i32,
    tx: &DatabaseTransaction,
    native_id_to_chain_id: &HashMap<String, i64>,
) -> Result<()> {
    let event = log
        .log_decode::<ITeleporterMessenger::ReceiveCrossChainMessage>()?
        .into_inner();
    let message_id_bytes = event.messageID.as_slice();
    let id = i64::from_be_bytes(message_id_bytes[..8].try_into()?);
    let tx_hash = log.transaction_hash.context("missing tx hash")?;
    let block_number = log.block_number.context("missing block number")?;
    let now = Utc::now().naive_utc();

    let source_blockchain_id_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());
    let mapped_src_chain_id = native_id_to_chain_id
        .get(&source_blockchain_id_hex)
        .copied();

    let Some(src_chain_id) = mapped_src_chain_id else {
        // tracing::warn!(
        //     source_blockchain_id = %source_blockchain_id_hex,
        //     "Source blockchain ID not found in chains table"
        // );
        return Ok(());
    };

    let message = crosschain_messages::ActiveModel {
        id: ActiveValue::Set(id),
        bridge_id: ActiveValue::Set(bridge_id),
        status: ActiveValue::Set(MessageStatus::Initiated),
        src_chain_id: ActiveValue::Set(src_chain_id),
        dst_chain_id: ActiveValue::Set(Some(chain_id)),
        dst_tx_hash: ActiveValue::Set(Some(tx_hash.as_slice().to_vec())),
        last_update_timestamp: ActiveValue::Set(Some(now)),
        sender_address: ActiveValue::Set(Some(
            event.message.originSenderAddress.as_slice().to_vec(),
        )),
        recipient_address: ActiveValue::Set(Some(
            event.message.destinationAddress.as_slice().to_vec(),
        )),
        payload: ActiveValue::Set(Some(event.message.message.to_vec())),
        updated_at: ActiveValue::Set(Some(Utc::now().naive_utc())),
        ..Default::default()
    };

    let update_columns = vec![
        crosschain_messages::Column::SrcChainId,
        crosschain_messages::Column::DstChainId,
        crosschain_messages::Column::DstTxHash,
        crosschain_messages::Column::LastUpdateTimestamp,
        crosschain_messages::Column::SenderAddress,
        crosschain_messages::Column::RecipientAddress,
        crosschain_messages::Column::Payload,
    ];

    upsert_message(tx, message, update_columns).await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        source_blockchain_id = %source_blockchain_id_hex,
        src_chain_id = ?mapped_src_chain_id,
        deliverer = %event.deliverer,
        nonce = %event.message.messageNonce,
        "Processed ReceiveCrossChainMessage"
    );

    Ok(())
}

async fn handle_message_executed(
    chain_id: i64,
    log: &Log,
    bridge_id: i32,
    tx: &DatabaseTransaction,
    native_id_to_chain_id: &HashMap<String, i64>,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::MessageExecuted>()?;
    let event = decoded.inner;
    let message_id_bytes = event.messageID.as_slice();
    let id = i64::from_be_bytes(message_id_bytes[..8].try_into()?);
    let tx_hash = log.transaction_hash;
    let block_number = log.block_number;
    let now = Utc::now().naive_utc();

    let source_blockchain_id_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());
    let mapped_src_chain_id = native_id_to_chain_id
        .get(&source_blockchain_id_hex)
        .copied();

    let Some(src_chain_id) = mapped_src_chain_id else {
        // tracing::warn!(
        //     source_blockchain_id = %source_blockchain_id_hex,
        //     "Source blockchain ID not found in chains table"
        // );
        return Ok(());
    };

    let mut message = crosschain_messages::ActiveModel {
        id: ActiveValue::Set(id),
        bridge_id: ActiveValue::Set(bridge_id),
        status: ActiveValue::Set(MessageStatus::Completed),
        src_chain_id: ActiveValue::Set(src_chain_id),
        dst_chain_id: ActiveValue::Set(Some(chain_id)),
        last_update_timestamp: ActiveValue::Set(Some(now)),
        updated_at: ActiveValue::Set(Some(Utc::now().naive_utc())),
        ..Default::default()
    };

    let mut update_columns = vec![
        crosschain_messages::Column::Status,
        crosschain_messages::Column::SrcChainId,
        crosschain_messages::Column::DstChainId,
        crosschain_messages::Column::LastUpdateTimestamp,
    ];

    if let Some(hash) = tx_hash {
        message.dst_tx_hash = ActiveValue::Set(Some(hash.as_slice().to_vec()));
        update_columns.push(crosschain_messages::Column::DstTxHash);
    }

    upsert_message(tx, message, update_columns).await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        source_blockchain_id = %source_blockchain_id_hex,
        "Processed MessageExecuted"
    );

    Ok(())
}

async fn handle_message_execution_failed(
    chain_id: i64,
    log: &Log,
    bridge_id: i32,
    tx: &DatabaseTransaction,
    native_id_to_chain_id: &HashMap<String, i64>,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::MessageExecutionFailed>()?;
    let event = decoded.inner;
    let message_id_bytes = event.messageID.as_slice();
    let id = i64::from_be_bytes(message_id_bytes[..8].try_into()?);
    let tx_hash = log.transaction_hash;
    let block_number = log.block_number;
    let now = Utc::now().naive_utc();

    let source_blockchain_id_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());
    let mapped_src_chain_id = native_id_to_chain_id
        .get(&source_blockchain_id_hex)
        .copied();

    let Some(src_chain_id) = mapped_src_chain_id else {
        // tracing::warn!(
        //     source_blockchain_id = %source_blockchain_id_hex,
        //     "Source blockchain ID not found in chains table"
        // );
        return Ok(());
    };

    let mut message = crosschain_messages::ActiveModel {
        id: ActiveValue::Set(id),
        bridge_id: ActiveValue::Set(bridge_id),
        status: ActiveValue::Set(MessageStatus::Failed),
        src_chain_id: ActiveValue::Set(src_chain_id),
        dst_chain_id: ActiveValue::Set(Some(chain_id)),
        last_update_timestamp: ActiveValue::Set(Some(now)),
        sender_address: ActiveValue::Set(Some(
            event.message.originSenderAddress.as_slice().to_vec(),
        )),
        recipient_address: ActiveValue::Set(Some(
            event.message.destinationAddress.as_slice().to_vec(),
        )),
        payload: ActiveValue::Set(Some(event.message.message.to_vec())),
        updated_at: ActiveValue::Set(Some(Utc::now().naive_utc())),
        ..Default::default()
    };

    let mut update_columns = vec![
        crosschain_messages::Column::Status,
        crosschain_messages::Column::SrcChainId,
        crosschain_messages::Column::DstChainId,
        crosschain_messages::Column::LastUpdateTimestamp,
        crosschain_messages::Column::SenderAddress,
        crosschain_messages::Column::RecipientAddress,
        crosschain_messages::Column::Payload,
    ];

    if let Some(hash) = tx_hash {
        message.dst_tx_hash = ActiveValue::Set(Some(hash.as_slice().to_vec()));
        update_columns.push(crosschain_messages::Column::DstTxHash);
    }

    upsert_message(tx, message, update_columns).await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        source_blockchain_id = %source_blockchain_id_hex,
        nonce = %event.message.messageNonce,
        "Processed MessageExecutionFailed"
    );

    Ok(())
}

// TODO: bug with max_block
async fn update_cursors(
    bridge_id: i32,
    chain_id: i64,
    max_block: i64,
    tx: &DatabaseTransaction,
) -> Result<(), DbErr> {
    indexer_checkpoints::Entity::insert(indexer_checkpoints::ActiveModel {
        bridge_id: ActiveValue::Set(bridge_id as i64),
        chain_id: ActiveValue::Set(chain_id),
        catchup_min_block: ActiveValue::Set(0),
        catchup_max_block: ActiveValue::Set(max_block),
        finality_cursor: ActiveValue::Set(0),
        realtime_cursor: ActiveValue::Set(max_block),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    })
    .on_conflict(
        OnConflict::columns([
            indexer_checkpoints::Column::BridgeId,
            indexer_checkpoints::Column::ChainId,
        ])
        .value(
            indexer_checkpoints::Column::CatchupMaxBlock,
            Expr::cust(
                "GREATEST(indexer_checkpoints.catchup_max_block, EXCLUDED.catchup_max_block)",
            ),
        )
        .value(
            indexer_checkpoints::Column::RealtimeCursor,
            Expr::cust("GREATEST(indexer_checkpoints.realtime_cursor, EXCLUDED.realtime_cursor)"),
        )
        .value(
            indexer_checkpoints::Column::UpdatedAt,
            SimpleExpr::from(Expr::current_timestamp()),
        )
        .to_owned(),
    )
    .exec(tx)
    .await?;

    tracing::debug!(bridge_id, chain_id, max_block, "Updated cursors");

    Ok(())
}
