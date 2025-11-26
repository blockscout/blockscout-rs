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
    crosschain_messages, indexer_checkpoints, pending_messages, sea_orm_active_enums::MessageStatus,
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

                let min_block = batch
                    .first()
                    .and_then(|log| log.block_number)
                    .ok_or_else(|| DbErr::Custom("missing block number".to_string()))?
                    as i64;
                let max_block = batch
                    .last()
                    .and_then(|log| log.block_number)
                    .ok_or_else(|| DbErr::Custom("missing block number".to_string()))?
                    as i64;

                update_cursors(bridge_id, chain_id, min_block, max_block, tx).await?;

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

    let destination_hex = blockchain_id_hex(event.destinationBlockchainID.as_slice());
    let dst_chain_id = native_id_to_chain_id.get(&destination_hex).copied();

    // Check if we have a pending destination-side message
    let pending = pending_messages::Entity::find_by_id((id, bridge_id))
        .one(tx)
        .await?;

    if let Some(pending_msg) = pending {
        // We have the destination side already - merge and insert complete message
        let dest_payload: serde_json::Value = pending_msg.payload.clone();

        let mut message = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(id),
            bridge_id: ActiveValue::Set(bridge_id),
            status: ActiveValue::Set(MessageStatus::Initiated),
            src_chain_id: ActiveValue::Set(chain_id),
            dst_chain_id: ActiveValue::Set(dst_chain_id),
            init_timestamp: ActiveValue::Set(now),
            last_update_timestamp: ActiveValue::Set(Some(now)),
            src_tx_hash: ActiveValue::Set(Some(tx_hash.as_slice().to_vec())),
            sender_address: ActiveValue::Set(Some(sender_bytes)),
            recipient_address: ActiveValue::Set(Some(recipient_bytes)),
            payload: ActiveValue::Set(Some(event.message.message.to_vec())),
            updated_at: ActiveValue::Set(Some(now)),
            ..Default::default()
        };

        // Extract destination data from pending payload
        if let Some(dst_tx_hash) = dest_payload.get("tx_hash").and_then(|v| v.as_str()) {
            if let Ok(hash_bytes) = hex::decode(dst_tx_hash.trim_start_matches("0x")) {
                message.dst_tx_hash = ActiveValue::Set(Some(hash_bytes));
            }
        }

        // Check if execution status is in the pending payload
        if let Some(status_str) = dest_payload.get("status").and_then(|v| v.as_str()) {
            let status = match status_str {
                "completed" => MessageStatus::Completed,
                "failed" => MessageStatus::Failed,
                _ => MessageStatus::Initiated,
            };
            message.status = ActiveValue::Set(status);
        }

        // Insert complete message
        crosschain_messages::Entity::insert(message)
            .exec(tx)
            .await?;

        // Delete from pending
        pending_messages::Entity::delete_by_id((id, bridge_id))
            .exec(tx)
            .await?;

        tracing::debug!(
            message_id = %hex::encode(message_id_bytes),
            chain_id,
            block_number,
            nonce = %event.message.messageNonce,
            "Promoted SendCrossChainMessage from pending to complete"
        );
    } else {
        // No pending destination - insert source-only message
        let message = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(id),
            bridge_id: ActiveValue::Set(bridge_id),
            status: ActiveValue::Set(MessageStatus::Initiated),
            src_chain_id: ActiveValue::Set(chain_id),
            dst_chain_id: ActiveValue::Set(dst_chain_id),
            init_timestamp: ActiveValue::Set(now),
            last_update_timestamp: ActiveValue::Set(Some(now)),
            src_tx_hash: ActiveValue::Set(Some(tx_hash.as_slice().to_vec())),
            sender_address: ActiveValue::Set(Some(sender_bytes)),
            recipient_address: ActiveValue::Set(Some(recipient_bytes)),
            payload: ActiveValue::Set(Some(event.message.message.to_vec())),
            updated_at: ActiveValue::Set(Some(now)),
            ..Default::default()
        };

        crosschain_messages::Entity::insert(message)
            .on_conflict(
                OnConflict::columns([
                    crosschain_messages::Column::Id,
                    crosschain_messages::Column::BridgeId,
                ])
                .update_columns([
                    crosschain_messages::Column::SrcChainId,
                    crosschain_messages::Column::SrcTxHash,
                    crosschain_messages::Column::SenderAddress,
                    crosschain_messages::Column::RecipientAddress,
                    crosschain_messages::Column::Payload,
                    crosschain_messages::Column::InitTimestamp,
                ])
                .to_owned(),
            )
            .exec(tx)
            .await?;

        tracing::debug!(
            message_id = %hex::encode(message_id_bytes),
            chain_id,
            block_number,
            nonce = %event.message.messageNonce,
            "Processed SendCrossChainMessage"
        );
    }

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
        // Source chain not in our config - skip
        return Ok(());
    };

    // Check if message already exists in crosschain_messages
    let existing = crosschain_messages::Entity::find_by_id((id, bridge_id))
        .one(tx)
        .await?;

    if let Some(_existing_msg) = existing {
        // Message already exists (source arrived first) - update with destination info
        let message = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(id),
            bridge_id: ActiveValue::Set(bridge_id),
            dst_chain_id: ActiveValue::Set(Some(chain_id)),
            dst_tx_hash: ActiveValue::Set(Some(tx_hash.as_slice().to_vec())),
            last_update_timestamp: ActiveValue::Set(Some(now)),
            updated_at: ActiveValue::Set(Some(now)),
            ..Default::default()
        };

        crosschain_messages::Entity::update(message)
            .exec(tx)
            .await?;

        tracing::debug!(
            message_id = %hex::encode(message_id_bytes),
            chain_id,
            block_number,
            source_blockchain_id = %source_blockchain_id_hex,
            src_chain_id,
            deliverer = %event.deliverer,
            nonce = %event.message.messageNonce,
            "Updated existing message with ReceiveCrossChainMessage"
        );
    } else {
        // Message doesn't exist yet - store in pending
        let payload = serde_json::json!({
            "event_type": "receive",
            "chain_id": chain_id,
            "block_number": block_number,
            "tx_hash": format!("0x{}", hex::encode(tx_hash.as_slice())),
            "source_blockchain_id": source_blockchain_id_hex,
            "sender": format!("0x{}", hex::encode(event.message.originSenderAddress.as_slice())),
            "recipient": format!("0x{}", hex::encode(event.message.destinationAddress.as_slice())),
            "message_payload": format!("0x{}", hex::encode(&event.message.message)),
            "nonce": event.message.messageNonce.to_string(),
            "deliverer": format!("0x{}", hex::encode(event.deliverer.as_slice())),
            "reward_redeemer": format!("0x{}", hex::encode(event.rewardRedeemer.as_slice())),
            "status": "initiated"
        });

        let pending = pending_messages::ActiveModel {
            message_id: ActiveValue::Set(id),
            bridge_id: ActiveValue::Set(bridge_id),
            payload: ActiveValue::Set(payload),
            created_at: ActiveValue::Set(Some(now)),
        };

        pending_messages::Entity::insert(pending)
            .on_conflict(
                OnConflict::columns([
                    pending_messages::Column::MessageId,
                    pending_messages::Column::BridgeId,
                ])
                .update_columns([pending_messages::Column::Payload])
                .to_owned(),
            )
            .exec(tx)
            .await?;

        tracing::debug!(
            message_id = %hex::encode(message_id_bytes),
            chain_id,
            block_number,
            source_blockchain_id = %source_blockchain_id_hex,
            src_chain_id,
            "Stored ReceiveCrossChainMessage in pending (waiting for source)"
        );
    }

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

    let Some(_src_chain_id) = mapped_src_chain_id else {
        return Ok(());
    };

    // Check if message exists
    let existing = crosschain_messages::Entity::find_by_id((id, bridge_id))
        .one(tx)
        .await?;

    if let Some(_) = existing {
        // Update existing message
        let mut message = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(id),
            bridge_id: ActiveValue::Set(bridge_id),
            status: ActiveValue::Set(MessageStatus::Completed),
            last_update_timestamp: ActiveValue::Set(Some(now)),
            updated_at: ActiveValue::Set(Some(now)),
            ..Default::default()
        };

        if let Some(hash) = tx_hash {
            message.dst_tx_hash = ActiveValue::Set(Some(hash.as_slice().to_vec()));
        }

        crosschain_messages::Entity::update(message)
            .exec(tx)
            .await?;

        tracing::debug!(
            message_id = %hex::encode(message_id_bytes),
            chain_id,
            block_number,
            source_blockchain_id = %source_blockchain_id_hex,
            "Updated message status to Completed"
        );
    } else {
        // Message doesn't exist - store in pending (execution implies receipt)
        let payload = serde_json::json!({
            "event_type": "executed",
            "chain_id": chain_id,
            "block_number": block_number.unwrap_or(0),
            "tx_hash": tx_hash.map(|h| format!("0x{}", hex::encode(h.as_slice()))).unwrap_or_default(),
            "source_blockchain_id": source_blockchain_id_hex,
            "status": "completed"
        });

        let pending = pending_messages::ActiveModel {
            message_id: ActiveValue::Set(id),
            bridge_id: ActiveValue::Set(bridge_id),
            payload: ActiveValue::Set(payload),
            created_at: ActiveValue::Set(Some(now)),
        };

        pending_messages::Entity::insert(pending)
            .on_conflict(
                OnConflict::columns([
                    pending_messages::Column::MessageId,
                    pending_messages::Column::BridgeId,
                ])
                .update_column(pending_messages::Column::Payload)
                .to_owned(),
            )
            .exec(tx)
            .await?;

        tracing::debug!(
            message_id = %hex::encode(message_id_bytes),
            chain_id,
            block_number,
            source_blockchain_id = %source_blockchain_id_hex,
            "Stored MessageExecuted in pending (waiting for source)"
        );
    }

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

    let Some(_src_chain_id) = mapped_src_chain_id else {
        return Ok(());
    };

    // Check if message exists
    let existing = crosschain_messages::Entity::find_by_id((id, bridge_id))
        .one(tx)
        .await?;

    if let Some(_) = existing {
        // Update existing message
        let mut message = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(id),
            bridge_id: ActiveValue::Set(bridge_id),
            status: ActiveValue::Set(MessageStatus::Failed),
            last_update_timestamp: ActiveValue::Set(Some(now)),
            sender_address: ActiveValue::Set(Some(
                event.message.originSenderAddress.as_slice().to_vec(),
            )),
            recipient_address: ActiveValue::Set(Some(
                event.message.destinationAddress.as_slice().to_vec(),
            )),
            payload: ActiveValue::Set(Some(event.message.message.to_vec())),
            updated_at: ActiveValue::Set(Some(now)),
            ..Default::default()
        };

        if let Some(hash) = tx_hash {
            message.dst_tx_hash = ActiveValue::Set(Some(hash.as_slice().to_vec()));
        }

        crosschain_messages::Entity::update(message)
            .exec(tx)
            .await?;

        tracing::debug!(
            message_id = %hex::encode(message_id_bytes),
            chain_id,
            block_number,
            source_blockchain_id = %source_blockchain_id_hex,
            nonce = %event.message.messageNonce,
            "Updated message status to Failed"
        );
    } else {
        // Message doesn't exist - store in pending
        let payload = serde_json::json!({
            "event_type": "failed",
            "chain_id": chain_id,
            "block_number": block_number.unwrap_or(0),
            "tx_hash": tx_hash.map(|h| format!("0x{}", hex::encode(h.as_slice()))).unwrap_or_default(),
            "source_blockchain_id": source_blockchain_id_hex,
            "sender": format!("0x{}", hex::encode(event.message.originSenderAddress.as_slice())),
            "recipient": format!("0x{}", hex::encode(event.message.destinationAddress.as_slice())),
            "message_payload": format!("0x{}", hex::encode(&event.message.message)),
            "nonce": event.message.messageNonce.to_string(),
            "status": "failed"
        });

        let pending = pending_messages::ActiveModel {
            message_id: ActiveValue::Set(id),
            bridge_id: ActiveValue::Set(bridge_id),
            payload: ActiveValue::Set(payload),
            created_at: ActiveValue::Set(Some(now)),
        };

        pending_messages::Entity::insert(pending)
            .on_conflict(
                OnConflict::columns([
                    pending_messages::Column::MessageId,
                    pending_messages::Column::BridgeId,
                ])
                .update_column(pending_messages::Column::Payload)
                .to_owned(),
            )
            .exec(tx)
            .await?;

        tracing::debug!(
            message_id = %hex::encode(message_id_bytes),
            chain_id,
            block_number,
            source_blockchain_id = %source_blockchain_id_hex,
            nonce = %event.message.messageNonce,
            "Stored MessageExecutionFailed in pending (waiting for source)"
        );
    }

    Ok(())
}

async fn update_cursors(
    bridge_id: i32,
    chain_id: i64,
    min_block: i64,
    max_block: i64,
    tx: &DatabaseTransaction,
) -> Result<(), DbErr> {
    indexer_checkpoints::Entity::insert(indexer_checkpoints::ActiveModel {
        bridge_id: ActiveValue::Set(bridge_id as i64),
        chain_id: ActiveValue::Set(chain_id),
        catchup_min_block: ActiveValue::Set(0),
        catchup_max_block: ActiveValue::Set(min_block),
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
            Expr::cust("LEAST(indexer_checkpoints.catchup_max_block, EXCLUDED.catchup_max_block)"),
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

    tracing::debug!(bridge_id, chain_id, min_block, max_block, "updated cursors");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_db;
    use alloy::{
        primitives::{Address, B256, Bytes, LogData, U256},
        rpc::types::Log,
    };
    use interchain_indexer_entity::{
        crosschain_messages, pending_messages, sea_orm_active_enums::MessageStatus,
    };
    use sea_orm::EntityTrait;

    // Helper to create a mock SendCrossChainMessage log
    fn create_send_message_log(
        message_id: [u8; 32],
        destination_blockchain_id: [u8; 32],
        block_number: u64,
        tx_hash: [u8; 32],
        nonce: u64,
    ) -> Log {
        let message = TeleporterMessage {
            messageNonce: U256::from(nonce),
            originSenderAddress: Address::repeat_byte(0x11),
            destinationBlockchainID: destination_blockchain_id.into(),
            destinationAddress: Address::repeat_byte(0x22),
            requiredGasLimit: U256::from(500000),
            allowedRelayerAddresses: vec![],
            receipts: vec![],
            message: Bytes::from(vec![0xab, 0xcd]),
        };

        let fee_info = TeleporterFeeInfo {
            feeTokenAddress: Address::ZERO,
            amount: U256::ZERO,
        };

        let event = ITeleporterMessenger::SendCrossChainMessage {
            messageID: message_id.into(),
            destinationBlockchainID: destination_blockchain_id.into(),
            message,
            feeInfo: fee_info,
        };

        let topics = vec![
            ITeleporterMessenger::SendCrossChainMessage::SIGNATURE_HASH,
            message_id.into(),
            destination_blockchain_id.into(),
        ];

        let data = event.encode_data();

        Log {
            inner: alloy::primitives::Log {
                address: Address::repeat_byte(0x25),
                data: LogData::new_unchecked(topics, data.into()),
            },
            block_hash: Some(B256::repeat_byte(0xbb)),
            block_number: Some(block_number),
            block_timestamp: None,
            transaction_hash: Some(B256::from(tx_hash)),
            transaction_index: Some(0),
            log_index: Some(0),
            removed: false,
        }
    }

    // Helper to create a mock ReceiveCrossChainMessage log
    fn create_receive_message_log(
        message_id: [u8; 32],
        source_blockchain_id: [u8; 32],
        block_number: u64,
        tx_hash: [u8; 32],
        nonce: u64,
    ) -> Log {
        let message = TeleporterMessage {
            messageNonce: U256::from(nonce),
            originSenderAddress: Address::repeat_byte(0x11),
            destinationBlockchainID: [0u8; 32].into(),
            destinationAddress: Address::repeat_byte(0x22),
            requiredGasLimit: U256::from(500000),
            allowedRelayerAddresses: vec![],
            receipts: vec![],
            message: Bytes::from(vec![0xab, 0xcd]),
        };

        let event = ITeleporterMessenger::ReceiveCrossChainMessage {
            messageID: message_id.into(),
            sourceBlockchainID: source_blockchain_id.into(),
            deliverer: Address::repeat_byte(0x33),
            rewardRedeemer: Address::repeat_byte(0x44),
            message,
        };

        let topics = vec![
            ITeleporterMessenger::ReceiveCrossChainMessage::SIGNATURE_HASH,
            message_id.into(),
            source_blockchain_id.into(),
            Address::repeat_byte(0x33).into_word(),
        ];

        let data = event.encode_data();

        Log {
            inner: alloy::primitives::Log {
                address: Address::repeat_byte(0x25),
                data: LogData::new_unchecked(topics, data.into()),
            },
            block_hash: Some(B256::repeat_byte(0xbb)),
            block_number: Some(block_number),
            block_timestamp: None,
            transaction_hash: Some(B256::from(tx_hash)),
            transaction_index: Some(0),
            log_index: Some(1),
            removed: false,
        }
    }

    // Helper to create a mock MessageExecuted log
    fn create_executed_message_log(
        message_id: [u8; 32],
        source_blockchain_id: [u8; 32],
        block_number: u64,
        tx_hash: [u8; 32],
    ) -> Log {
        let event = ITeleporterMessenger::MessageExecuted {
            messageID: message_id.into(),
            sourceBlockchainID: source_blockchain_id.into(),
        };

        let topics = vec![
            ITeleporterMessenger::MessageExecuted::SIGNATURE_HASH,
            message_id.into(),
            source_blockchain_id.into(),
        ];

        let data = event.encode_data();

        Log {
            inner: alloy::primitives::Log {
                address: Address::repeat_byte(0x25),
                data: LogData::new_unchecked(topics, data.into()),
            },
            block_hash: Some(B256::repeat_byte(0xbb)),
            block_number: Some(block_number),
            block_timestamp: None,
            transaction_hash: Some(B256::from(tx_hash)),
            transaction_index: Some(0),
            log_index: Some(2),
            removed: false,
        }
    }

    // Helper to create a mock MessageExecutionFailed log
    fn create_failed_message_log(
        message_id: [u8; 32],
        source_blockchain_id: [u8; 32],
        block_number: u64,
        tx_hash: [u8; 32],
        nonce: u64,
    ) -> Log {
        let message = TeleporterMessage {
            messageNonce: U256::from(nonce),
            originSenderAddress: Address::repeat_byte(0x11),
            destinationBlockchainID: [0u8; 32].into(),
            destinationAddress: Address::repeat_byte(0x22),
            requiredGasLimit: U256::from(500000),
            allowedRelayerAddresses: vec![],
            receipts: vec![],
            message: Bytes::from(vec![0xab, 0xcd]),
        };

        let event = ITeleporterMessenger::MessageExecutionFailed {
            messageID: message_id.into(),
            sourceBlockchainID: source_blockchain_id.into(),
            message,
        };

        let topics = vec![
            ITeleporterMessenger::MessageExecutionFailed::SIGNATURE_HASH,
            message_id.into(),
            source_blockchain_id.into(),
        ];

        let data = event.encode_data();

        Log {
            inner: alloy::primitives::Log {
                address: Address::repeat_byte(0x25),
                data: LogData::new_unchecked(topics, data.into()),
            },
            block_hash: Some(B256::repeat_byte(0xbb)),
            block_number: Some(block_number),
            block_timestamp: None,
            transaction_hash: Some(B256::from(tx_hash)),
            transaction_index: Some(0),
            log_index: Some(2),
            removed: false,
        }
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_sequential_event_flow() {
        // Setup database
        let db = init_db("avax_seq_flow").await;
        let interchain_db = InterchainDatabase::new(db.client());

        // Setup test data
        let bridge_id = 1i32;
        let src_chain_id = 43114i64; // Avalanche C-Chain
        let dst_chain_id = 43113i64; // Fuji testnet

        // Create chains
        interchain_db
            .upsert_chains(vec![
                interchain_indexer_entity::chains::ActiveModel {
                    id: ActiveValue::Set(src_chain_id),
                    name: ActiveValue::Set("Avalanche C-Chain".to_string()),
                    native_id: ActiveValue::Set(Some("0x".to_string() + &hex::encode([1u8; 32]))),
                    ..Default::default()
                },
                interchain_indexer_entity::chains::ActiveModel {
                    id: ActiveValue::Set(dst_chain_id),
                    name: ActiveValue::Set("Fuji Testnet".to_string()),
                    native_id: ActiveValue::Set(Some("0x".to_string() + &hex::encode([2u8; 32]))),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        // Create bridge
        interchain_db
            .upsert_bridges(vec![interchain_indexer_entity::bridges::ActiveModel {
                id: ActiveValue::Set(bridge_id),
                name: ActiveValue::Set("Teleporter".to_string()),
                enabled: ActiveValue::Set(true),
                ..Default::default()
            }])
            .await
            .unwrap();

        let native_id_to_chain_id = interchain_db.load_native_id_map().await.unwrap();

        // Test message ID
        let mut message_id_bytes = [0u8; 32];
        message_id_bytes[0..8].copy_from_slice(&123456789i64.to_be_bytes());

        let src_blockchain_id = [1u8; 32];
        let dst_blockchain_id = [2u8; 32];

        // Step 1: Process SendCrossChainMessage
        let send_log =
            create_send_message_log(message_id_bytes, dst_blockchain_id, 100, [0x1a; 32], 1);

        let tx = interchain_db.db.begin().await.unwrap();
        handle_send_cross_chain_message(
            src_chain_id,
            &send_log,
            bridge_id,
            &tx,
            &native_id_to_chain_id,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Verify message was created with init_timestamp
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, 123456789i64);
        assert_eq!(messages[0].status, MessageStatus::Initiated);
        assert_eq!(messages[0].src_chain_id, src_chain_id);
        assert!(messages[0].dst_tx_hash.is_none());

        // No pending messages should exist
        let pending = pending_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(pending.len(), 0);

        // Step 2: Process ReceiveCrossChainMessage
        let receive_log =
            create_receive_message_log(message_id_bytes, src_blockchain_id, 200, [0x2b; 32], 1);

        let tx = interchain_db.db.begin().await.unwrap();
        handle_receive_cross_chain_message(
            dst_chain_id,
            &receive_log,
            bridge_id,
            &tx,
            &native_id_to_chain_id,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Verify message was updated with destination info
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].dst_chain_id, Some(dst_chain_id));
        assert!(messages[0].dst_tx_hash.is_some());
        assert_eq!(messages[0].status, MessageStatus::Initiated);

        // Step 3: Process MessageExecuted
        let executed_log =
            create_executed_message_log(message_id_bytes, src_blockchain_id, 201, [0x3c; 32]);

        let tx = interchain_db.db.begin().await.unwrap();
        handle_message_executed(
            dst_chain_id,
            &executed_log,
            bridge_id,
            &tx,
            &native_id_to_chain_id,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Verify message status was updated to Completed
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].status, MessageStatus::Completed);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_out_of_order_receive_before_send() {
        // Setup database
        let db = init_db("avax_out_of_order").await;
        let interchain_db = InterchainDatabase::new(db.client());

        // Setup test data
        let bridge_id = 1i32;
        let src_chain_id = 43114i64;
        let dst_chain_id = 43113i64;

        // Create chains
        interchain_db
            .upsert_chains(vec![
                interchain_indexer_entity::chains::ActiveModel {
                    id: ActiveValue::Set(src_chain_id),
                    name: ActiveValue::Set("Avalanche C-Chain".to_string()),
                    native_id: ActiveValue::Set(Some("0x".to_string() + &hex::encode([1u8; 32]))),
                    ..Default::default()
                },
                interchain_indexer_entity::chains::ActiveModel {
                    id: ActiveValue::Set(dst_chain_id),
                    name: ActiveValue::Set("Fuji Testnet".to_string()),
                    native_id: ActiveValue::Set(Some("0x".to_string() + &hex::encode([2u8; 32]))),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        // Create bridge
        interchain_db
            .upsert_bridges(vec![interchain_indexer_entity::bridges::ActiveModel {
                id: ActiveValue::Set(bridge_id),
                name: ActiveValue::Set("Teleporter".to_string()),
                enabled: ActiveValue::Set(true),
                ..Default::default()
            }])
            .await
            .unwrap();

        let native_id_to_chain_id = interchain_db.load_native_id_map().await.unwrap();

        let mut message_id_bytes = [0u8; 32];
        message_id_bytes[0..8].copy_from_slice(&987654321i64.to_be_bytes());

        let src_blockchain_id = [1u8; 32];
        let dst_blockchain_id = [2u8; 32];

        // Step 1: Process ReceiveCrossChainMessage FIRST (out of order)
        let receive_log =
            create_receive_message_log(message_id_bytes, src_blockchain_id, 200, [0x2b; 32], 1);

        let tx = interchain_db.db.begin().await.unwrap();
        handle_receive_cross_chain_message(
            dst_chain_id,
            &receive_log,
            bridge_id,
            &tx,
            &native_id_to_chain_id,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Verify message was NOT created in crosschain_messages
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(messages.len(), 0, "Message should not exist yet");

        // Verify pending message was created
        let pending = pending_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].message_id, 987654321i64);
        assert_eq!(pending[0].bridge_id, bridge_id);

        // Step 2: Process SendCrossChainMessage (arrives late)
        let send_log =
            create_send_message_log(message_id_bytes, dst_blockchain_id, 100, [0x1a; 32], 1);

        let tx = interchain_db.db.begin().await.unwrap();
        handle_send_cross_chain_message(
            src_chain_id,
            &send_log,
            bridge_id,
            &tx,
            &native_id_to_chain_id,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Verify message was promoted to crosschain_messages
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, 987654321i64);
        assert_eq!(messages[0].status, MessageStatus::Initiated);
        assert_eq!(messages[0].src_chain_id, src_chain_id);
        assert_eq!(messages[0].dst_chain_id, Some(dst_chain_id));
        assert!(messages[0].src_tx_hash.is_some());
        assert!(messages[0].dst_tx_hash.is_some());

        // Verify pending message was deleted
        let pending = pending_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(
            pending.len(),
            0,
            "Pending message should be deleted after promotion"
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_execution_before_send() {
        // Setup database
        let db = init_db("avax_exec_first").await;
        let interchain_db = InterchainDatabase::new(db.client());

        let bridge_id = 1i32;
        let src_chain_id = 43114i64;
        let dst_chain_id = 43113i64;

        // Create chains
        interchain_db
            .upsert_chains(vec![
                interchain_indexer_entity::chains::ActiveModel {
                    id: ActiveValue::Set(src_chain_id),
                    name: ActiveValue::Set("Avalanche C-Chain".to_string()),
                    native_id: ActiveValue::Set(Some("0x".to_string() + &hex::encode([1u8; 32]))),
                    ..Default::default()
                },
                interchain_indexer_entity::chains::ActiveModel {
                    id: ActiveValue::Set(dst_chain_id),
                    name: ActiveValue::Set("Fuji Testnet".to_string()),
                    native_id: ActiveValue::Set(Some("0x".to_string() + &hex::encode([2u8; 32]))),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        interchain_db
            .upsert_bridges(vec![interchain_indexer_entity::bridges::ActiveModel {
                id: ActiveValue::Set(bridge_id),
                name: ActiveValue::Set("Teleporter".to_string()),
                enabled: ActiveValue::Set(true),
                ..Default::default()
            }])
            .await
            .unwrap();

        let native_id_to_chain_id = interchain_db.load_native_id_map().await.unwrap();

        let mut message_id_bytes = [0u8; 32];
        message_id_bytes[0..8].copy_from_slice(&111222333i64.to_be_bytes());

        let src_blockchain_id = [1u8; 32];
        let dst_blockchain_id = [2u8; 32];

        // Step 1: Process MessageExecuted FIRST (very out of order)
        let executed_log =
            create_executed_message_log(message_id_bytes, src_blockchain_id, 201, [0x3c; 32]);

        let tx = interchain_db.db.begin().await.unwrap();
        handle_message_executed(
            dst_chain_id,
            &executed_log,
            bridge_id,
            &tx,
            &native_id_to_chain_id,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Should be in pending with status=completed
        let pending = pending_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(pending.len(), 1);
        let payload: serde_json::Value = pending[0].payload.clone();
        assert_eq!(
            payload.get("status").and_then(|v| v.as_str()),
            Some("completed")
        );

        // Step 2: Process SendCrossChainMessage
        let send_log =
            create_send_message_log(message_id_bytes, dst_blockchain_id, 100, [0x1a; 32], 1);

        let tx = interchain_db.db.begin().await.unwrap();
        handle_send_cross_chain_message(
            src_chain_id,
            &send_log,
            bridge_id,
            &tx,
            &native_id_to_chain_id,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Should be promoted with Completed status
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].status, MessageStatus::Completed);

        let pending = pending_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_failed_execution_flow() {
        let db = init_db("avax_failed_exec").await;
        let interchain_db = InterchainDatabase::new(db.client());

        let bridge_id = 1i32;
        let src_chain_id = 43114i64;
        let dst_chain_id = 43113i64;

        interchain_db
            .upsert_chains(vec![
                interchain_indexer_entity::chains::ActiveModel {
                    id: ActiveValue::Set(src_chain_id),
                    name: ActiveValue::Set("Avalanche C-Chain".to_string()),
                    native_id: ActiveValue::Set(Some("0x".to_string() + &hex::encode([1u8; 32]))),
                    ..Default::default()
                },
                interchain_indexer_entity::chains::ActiveModel {
                    id: ActiveValue::Set(dst_chain_id),
                    name: ActiveValue::Set("Fuji Testnet".to_string()),
                    native_id: ActiveValue::Set(Some("0x".to_string() + &hex::encode([2u8; 32]))),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        interchain_db
            .upsert_bridges(vec![interchain_indexer_entity::bridges::ActiveModel {
                id: ActiveValue::Set(bridge_id),
                name: ActiveValue::Set("Teleporter".to_string()),
                enabled: ActiveValue::Set(true),
                ..Default::default()
            }])
            .await
            .unwrap();

        let native_id_to_chain_id = interchain_db.load_native_id_map().await.unwrap();

        let mut message_id_bytes = [0u8; 32];
        message_id_bytes[0..8].copy_from_slice(&444555666i64.to_be_bytes());

        let src_blockchain_id = [1u8; 32];
        let dst_blockchain_id = [2u8; 32];

        // Step 1: Send
        let send_log =
            create_send_message_log(message_id_bytes, dst_blockchain_id, 100, [0x1a; 32], 1);

        let tx = interchain_db.db.begin().await.unwrap();
        handle_send_cross_chain_message(
            src_chain_id,
            &send_log,
            bridge_id,
            &tx,
            &native_id_to_chain_id,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Step 2: Receive
        let receive_log =
            create_receive_message_log(message_id_bytes, src_blockchain_id, 200, [0x2b; 32], 1);

        let tx = interchain_db.db.begin().await.unwrap();
        handle_receive_cross_chain_message(
            dst_chain_id,
            &receive_log,
            bridge_id,
            &tx,
            &native_id_to_chain_id,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Step 3: Execution Failed
        let failed_log =
            create_failed_message_log(message_id_bytes, src_blockchain_id, 201, [0x3c; 32], 1);

        let tx = interchain_db.db.begin().await.unwrap();
        handle_message_execution_failed(
            dst_chain_id,
            &failed_log,
            bridge_id,
            &tx,
            &native_id_to_chain_id,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        // Verify final state is Failed
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].status, MessageStatus::Failed);
    }
}
