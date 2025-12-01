use std::{collections::HashMap, sync::Arc, time::Duration};

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
use tokio::task::JoinHandle;

use crate::{
    InterchainDatabase,
    log_stream::LogStreamBuilder,
    message_buffer::{Config, Key, MessageBuffer, Status},
};

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
    buffer: Arc<MessageBuffer>,
}

impl AvalancheIndexer {
    fn new(db: InterchainDatabase, config: AvalancheIndexerConfig) -> Result<Self> {
        if config.chains.is_empty() {
            return Err(anyhow!(
                "Avalanche indexer requires at least one configured chain"
            ));
        }

        let buffer_config = Config::default();
        let buffer = MessageBuffer::new(db.clone(), buffer_config);

        Ok(Self { db, config, buffer })
    }

    async fn run(self) -> Result<()> {
        let AvalancheIndexer { db, config, buffer } = self;
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
                (latest_block, latest_block.saturating_sub(1))
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

        let buffer_handle = Arc::clone(&buffer).start().await?;

        // Process events
        while let Some((chain_id, batch)) = combined_stream.next().await {
            match process_batch(batch, chain_id, bridge_id, &native_id_to_chain_id, &buffer).await {
                Ok(()) => {
                    tracing::debug!(bridge_id, chain_id, "Processed Avalanche log batch");
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

        buffer_handle.abort();
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
    buffer: &Arc<MessageBuffer>,
) -> Result<()> {
    for log in &batch {
        let block_number = log.block_number.context("missing block number")? as i64;

        handle_log(
            chain_id,
            block_number,
            log,
            bridge_id,
            native_id_to_chain_id,
            buffer,
        )
        .await?;
    }

    Ok(())
}

async fn handle_log(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer>,
) -> anyhow::Result<()> {
    match log.topic0() {
        Some(&ITeleporterMessenger::SendCrossChainMessage::SIGNATURE_HASH) => {
            handle_send_cross_chain_message(
                chain_id,
                block_number,
                log,
                bridge_id,
                native_id_to_chain_id,
                buffer,
            )
            .await
        }
        Some(&ITeleporterMessenger::ReceiveCrossChainMessage::SIGNATURE_HASH) => {
            handle_receive_cross_chain_message(
                chain_id,
                block_number,
                log,
                bridge_id,
                native_id_to_chain_id,
                buffer,
            )
            .await
        }
        Some(&ITeleporterMessenger::MessageExecuted::SIGNATURE_HASH) => {
            handle_message_executed(
                chain_id,
                block_number,
                log,
                bridge_id,
                native_id_to_chain_id,
                buffer,
            )
            .await
        }
        Some(&ITeleporterMessenger::MessageExecutionFailed::SIGNATURE_HASH) => {
            handle_message_execution_failed(
                chain_id,
                block_number,
                log,
                bridge_id,
                native_id_to_chain_id,
                buffer,
            )
            .await
        }
        _ => Err(anyhow::Error::msg("unknown event")),
    }
}

/// Handle SendCrossChainMessage - this is the SOURCE event that makes messages "ready"
async fn handle_send_cross_chain_message(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer>,
) -> Result<()> {
    let event = log
        .log_decode::<ITeleporterMessenger::SendCrossChainMessage>()?
        .inner;
    let message_id_bytes = event.messageID.as_slice();
    let id = i64::from_be_bytes(message_id_bytes[..8].try_into()?);
    let tx_hash = log.transaction_hash.context("missing tx hash")?;
    let now = Utc::now().naive_utc();

    let destination_hex = blockchain_id_hex(event.destinationBlockchainID.as_slice());
    let dst_chain_id = native_id_to_chain_id.get(&destination_hex).copied();

    // TODO: Support messages to untracked chains in the future.
    // For now, skip messages going to chains we don't index.
    if dst_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            destination_blockchain_id = %destination_hex,
            "Skipping SendCrossChainMessage to untracked chain"
        );
        return Ok(());
    }

    let key = Key::new(id, bridge_id);

    // Get or create entry from buffer (checks hot tier, then cold tier, then creates new)
    let mut entry = buffer.get_or_create(key).await?;

    // Fill in source-side data
    entry.src_chain_id = Some(chain_id);
    entry.source_transaction_hash = Some(tx_hash.into());
    entry.init_timestamp = Some(now); // This makes the message "ready"
    entry.sender_address = Some(event.message.originSenderAddress);
    entry.recipient_address = Some(event.message.destinationAddress);
    entry.destination_chain_id = dst_chain_id;
    entry.native_id = Some(message_id_bytes.to_vec());
    entry.payload = Some(event.message.message.to_vec());
    entry.cursor.record_block(chain_id, block_number);

    // Update buffer (message is now ready and will be flushed on next maintenance)
    buffer.upsert(entry).await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        nonce = %event.message.messageNonce,
        is_ready = true,
        "Processed SendCrossChainMessage"
    );

    Ok(())
}

/// Handle ReceiveCrossChainMessage - destination event, may arrive before source
async fn handle_receive_cross_chain_message(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer>,
) -> Result<()> {
    let event = log
        .log_decode::<ITeleporterMessenger::ReceiveCrossChainMessage>()?
        .into_inner();
    let message_id_bytes = event.messageID.as_slice();
    let id = i64::from_be_bytes(message_id_bytes[..8].try_into()?);
    let tx_hash = log.transaction_hash.context("missing tx hash")?;

    let source_blockchain_id_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());
    let src_chain_id = native_id_to_chain_id
        .get(&source_blockchain_id_hex)
        .copied();

    // TODO: Support messages from untracked chains in the future.
    // For now, skip messages coming from chains we don't index.
    if src_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_blockchain_id_hex,
            "Skipping ReceiveCrossChainMessage from untracked chain"
        );
        return Ok(());
    }

    let key = Key::new(id, bridge_id);
    let mut entry = buffer.get_or_create(key).await?;

    // Fill in destination-side data
    entry.destination_chain_id = Some(chain_id);
    entry.destination_transaction_hash = Some(tx_hash.into());
    entry.native_id = Some(message_id_bytes.to_vec());
    entry.sender_address = event.message.originSenderAddress.into();
    entry.recipient_address = event.message.destinationAddress.into();
    entry.payload = event.message.message.to_vec().into();
    entry.src_chain_id = src_chain_id;
    entry.cursor.record_block(chain_id, block_number);

    buffer.upsert(entry).await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        source_blockchain_id = %source_blockchain_id_hex,
        nonce = %event.message.messageNonce,
        "Processed ReceiveCrossChainMessage"
    );

    Ok(())
}

/// Handle MessageExecuted - marks message as completed
async fn handle_message_executed(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer>,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::MessageExecuted>()?;
    let event = decoded.inner;
    let message_id_bytes = event.messageID.as_slice();
    let id = i64::from_be_bytes(message_id_bytes[..8].try_into()?);

    let source_blockchain_id_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());
    let src_chain_id = native_id_to_chain_id
        .get(&source_blockchain_id_hex)
        .copied();

    // TODO: Support messages from untracked chains in the future.
    // For now, skip messages coming from chains we don't index.
    if src_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_blockchain_id_hex,
            "Skipping MessageExecuted from untracked chain"
        );
        return Ok(());
    }

    let key = Key::new(id, bridge_id);
    let mut entry = buffer.get_or_create(key).await?;

    // Mark as completed
    entry.status = Status::Completed;
    entry.destination_chain_id = Some(chain_id);
    entry.native_id = Some(message_id_bytes.to_vec());
    entry.destination_transaction_hash = log.transaction_hash;
    entry.src_chain_id = src_chain_id;
    entry.cursor.record_block(chain_id, block_number);

    buffer.upsert(entry).await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        source_blockchain_id = %source_blockchain_id_hex,
        "Processed MessageExecuted"
    );

    Ok(())
}

/// Handle MessageExecutionFailed - marks message as failed
async fn handle_message_execution_failed(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer>,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::MessageExecutionFailed>()?;
    let event = decoded.inner;
    let message_id_bytes = event.messageID.as_slice();
    let id = i64::from_be_bytes(message_id_bytes[..8].try_into()?);

    let source_blockchain_id_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());
    let src_chain_id = native_id_to_chain_id
        .get(&source_blockchain_id_hex)
        .copied();

    // TODO: Support messages from untracked chains in the future.
    // For now, skip messages coming from chains we don't index.
    if src_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_blockchain_id_hex,
            "Skipping MessageExecutionFailed from untracked chain"
        );
        return Ok(());
    }

    let key = Key::new(id, bridge_id);
    let mut entry = buffer.get_or_create(key).await?;

    // Mark as failed
    entry.status = Status::Failed;
    entry.destination_chain_id = Some(chain_id);
    entry.native_id = Some(message_id_bytes.to_vec());
    entry.sender_address = Some(event.message.originSenderAddress);
    entry.recipient_address = Some(event.message.destinationAddress);
    entry.payload = Some(event.message.message.to_vec());
    entry.destination_transaction_hash = log.transaction_hash;
    entry.src_chain_id = src_chain_id;
    entry.cursor.record_block(chain_id, block_number);

    buffer.upsert(entry).await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_db;
    use alloy::{
        primitives::{Address, B256, Bytes, LogData, U256},
        rpc::types::Log,
    };
    use interchain_indexer_entity::{crosschain_messages, sea_orm_active_enums::MessageStatus};
    use sea_orm::{ActiveValue, EntityTrait};

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

    /// Helper to set up test environment with chains and bridge
    async fn setup_test_env(
        test_name: &str,
    ) -> (
        InterchainDatabase,
        Arc<MessageBuffer>,
        HashMap<String, i64>,
        i32, // bridge_id
        i64, // src_chain_id
        i64, // dst_chain_id
    ) {
        let db = init_db(test_name).await;
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

        let buffer = MessageBuffer::new(interchain_db.clone(), Config::default());

        (
            interchain_db,
            buffer,
            native_id_to_chain_id,
            bridge_id,
            src_chain_id,
            dst_chain_id,
        )
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_sequential_event_flow_with_buffer() {
        let (interchain_db, buffer, native_id_to_chain_id, bridge_id, src_chain_id, dst_chain_id) =
            setup_test_env("avax_buf_seq").await;

        let mut message_id_bytes = [0u8; 32];
        message_id_bytes[0..8].copy_from_slice(&123456789i64.to_be_bytes());

        let src_blockchain_id = [1u8; 32];
        let dst_blockchain_id = [2u8; 32];

        // Step 1: Process SendCrossChainMessage
        let send_log =
            create_send_message_log(message_id_bytes, dst_blockchain_id, 100, [0x1a; 32], 1);

        handle_send_cross_chain_message(
            src_chain_id,
            100,
            &send_log,
            bridge_id,
            &native_id_to_chain_id,
            &buffer,
        )
        .await
        .unwrap();

        // Message should be in hot tier (ready = has init_timestamp)
        let hot_count = buffer.hot_len();
        assert_eq!(hot_count, 1);

        // Step 2: Process ReceiveCrossChainMessage
        let receive_log =
            create_receive_message_log(message_id_bytes, src_blockchain_id, 200, [0x2b; 32], 1);

        handle_receive_cross_chain_message(
            dst_chain_id,
            200,
            &receive_log,
            bridge_id,
            &native_id_to_chain_id,
            &buffer,
        )
        .await
        .unwrap();

        // Step 3: Process MessageExecuted
        let executed_log =
            create_executed_message_log(message_id_bytes, src_blockchain_id, 201, [0x3c; 32]);

        handle_message_executed(
            dst_chain_id,
            201,
            &executed_log,
            bridge_id,
            &native_id_to_chain_id,
            &buffer,
        )
        .await
        .unwrap();

        // Run maintenance to flush to DB
        buffer.run().await.unwrap();

        // Verify message in DB with Completed status
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, 123456789i64);
        assert_eq!(messages[0].status, MessageStatus::Completed);
        assert_eq!(messages[0].src_chain_id, src_chain_id);
        assert_eq!(messages[0].dst_chain_id, Some(dst_chain_id));
        assert!(messages[0].src_tx_hash.is_some());
        assert!(messages[0].dst_tx_hash.is_some());
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_out_of_order_with_buffer() {
        let (interchain_db, buffer, native_id_to_chain_id, bridge_id, src_chain_id, dst_chain_id) =
            setup_test_env("avax_buf_ooo").await;

        let mut message_id_bytes = [0u8; 32];
        message_id_bytes[0..8].copy_from_slice(&987654321i64.to_be_bytes());

        let src_blockchain_id = [1u8; 32];
        let dst_blockchain_id = [2u8; 32];

        // Step 1: Process ReceiveCrossChainMessage FIRST (out of order)
        let receive_log =
            create_receive_message_log(message_id_bytes, src_blockchain_id, 200, [0x2b; 32], 1);

        handle_receive_cross_chain_message(
            dst_chain_id,
            200,
            &receive_log,
            bridge_id,
            &native_id_to_chain_id,
            &buffer,
        )
        .await
        .unwrap();

        // Message should be in hot tier but NOT ready (no init_timestamp)
        let key = Key::new(987654321i64, bridge_id);
        let entry = buffer.get_or_create(key).await.unwrap();
        assert!(!entry.is_ready());
        assert!(entry.destination_transaction_hash.is_some());

        // Run maintenance - should NOT flush (message not ready)
        buffer.run().await.unwrap();

        // Step 2: Process SendCrossChainMessage (arrives late)
        let send_log =
            create_send_message_log(message_id_bytes, dst_blockchain_id, 100, [0x1a; 32], 1);

        handle_send_cross_chain_message(
            src_chain_id,
            100,
            &send_log,
            bridge_id,
            &native_id_to_chain_id,
            &buffer,
        )
        .await
        .unwrap();

        // Now message should be ready
        let entry = buffer.get_or_create(key).await.unwrap();
        assert!(entry.is_ready());
        assert!(entry.source_transaction_hash.is_some());
        assert!(entry.destination_transaction_hash.is_some());

        // Run maintenance - should flush now
        buffer.run().await.unwrap();

        // Verify message in DB
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, 987654321i64);
        assert_eq!(messages[0].src_chain_id, src_chain_id);
        assert_eq!(messages[0].dst_chain_id, Some(dst_chain_id));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_execution_before_send_with_buffer() {
        let (interchain_db, buffer, native_id_to_chain_id, bridge_id, src_chain_id, dst_chain_id) =
            setup_test_env("avax_buf_exec_first").await;

        let mut message_id_bytes = [0u8; 32];
        message_id_bytes[0..8].copy_from_slice(&111222333i64.to_be_bytes());

        let src_blockchain_id = [1u8; 32];
        let dst_blockchain_id = [2u8; 32];

        // Step 1: Process MessageExecuted FIRST (very out of order)
        let executed_log =
            create_executed_message_log(message_id_bytes, src_blockchain_id, 201, [0x3c; 32]);

        handle_message_executed(
            dst_chain_id,
            201,
            &executed_log,
            bridge_id,
            &native_id_to_chain_id,
            &buffer,
        )
        .await
        .unwrap();

        // Message not ready yet
        let key = Key::new(111222333i64, bridge_id);
        let entry = buffer.get_or_create(key).await.unwrap();
        assert!(!entry.is_ready());
        assert_eq!(entry.status, Status::Completed);

        // Step 2: Process SendCrossChainMessage
        let send_log =
            create_send_message_log(message_id_bytes, dst_blockchain_id, 100, [0x1a; 32], 1);

        handle_send_cross_chain_message(
            src_chain_id,
            100,
            &send_log,
            bridge_id,
            &native_id_to_chain_id,
            &buffer,
        )
        .await
        .unwrap();

        // Run maintenance
        buffer.run().await.unwrap();

        // Verify final state - should be Completed
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].status, MessageStatus::Completed);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_failed_execution_with_buffer() {
        let (interchain_db, buffer, native_id_to_chain_id, bridge_id, src_chain_id, dst_chain_id) =
            setup_test_env("avax_buf_failed").await;

        let mut message_id_bytes = [0u8; 32];
        message_id_bytes[0..8].copy_from_slice(&444555666i64.to_be_bytes());

        let src_blockchain_id = [1u8; 32];
        let dst_blockchain_id = [2u8; 32];

        // Step 1: Send
        let send_log =
            create_send_message_log(message_id_bytes, dst_blockchain_id, 100, [0x1a; 32], 1);
        handle_send_cross_chain_message(
            src_chain_id,
            100,
            &send_log,
            bridge_id,
            &native_id_to_chain_id,
            &buffer,
        )
        .await
        .unwrap();

        // Step 2: Receive
        let receive_log =
            create_receive_message_log(message_id_bytes, src_blockchain_id, 200, [0x2b; 32], 1);
        handle_receive_cross_chain_message(
            dst_chain_id,
            200,
            &receive_log,
            bridge_id,
            &native_id_to_chain_id,
            &buffer,
        )
        .await
        .unwrap();

        // Step 3: Execution Failed
        let failed_log =
            create_failed_message_log(message_id_bytes, src_blockchain_id, 201, [0x3c; 32], 1);
        handle_message_execution_failed(
            dst_chain_id,
            201,
            &failed_log,
            bridge_id,
            &native_id_to_chain_id,
            &buffer,
        )
        .await
        .unwrap();

        // Run maintenance
        buffer.run().await.unwrap();

        // Verify final state is Failed
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].status, MessageStatus::Failed);
    }
}
