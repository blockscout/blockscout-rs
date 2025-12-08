use std::{collections::HashMap, sync::Arc, time::Duration};

use alloy::{
    network::Ethereum,
    primitives::{Address, B256},
    providers::{DynProvider, Provider as _},
    rpc::types::{Filter, Log},
    sol,
    sol_types::SolEvent,
};
use anyhow::{Context, Result, anyhow};
use futures::{StreamExt, stream};
use tokio::task::JoinHandle;

use crate::{
    InterchainDatabase,
    log_stream::LogStreamBuilder,
    message_buffer::{Config, IcttEventFragment, Key, MessageBuffer, Status},
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

    /// @notice Input parameters for transferring tokens to another chain as
    /// part of a simple transfer.
    ///
    /// @param destinationBlockchainID Blockchain ID of the destination
    ///
    /// @param destinationTokenTransferrerAddress Address of the destination
    /// token transferrer instance
    ///
    /// @param recipient Address of the recipient on the destination chain
    ///
    /// @param primaryFeeTokenAddress Address of the ERC20 contract to
    /// optionally pay a Teleporter message fee
    ///
    /// @param primaryFee Amount of tokens to pay as the optional Teleporter
    /// message fee
    ///
    /// @param secondaryFee Amount of tokens to pay for Teleporter fee if a
    /// multi-hop is needed
    ///
    /// @param requiredGasLimit Gas limit requirement for sending to a token
    /// transferrer. This is required because the gas requirement varies based
    /// on the token transferrer instance specified by
    /// {destinationBlockchainID} and {destinationTokenTransferrerAddress}.
    ///
    ///
    /// @param multiHopFallback In the case of a multi-hop transfer, the
    /// address where the tokens are sent on the home chain if the transfer is
    /// unable to be routed to its final destination. Note that this address
    /// must be able to receive the tokens held as collateral in the home
    /// contract.
    struct SendTokensInput {
        bytes32 destinationBlockchainID;
        address destinationTokenTransferrerAddress;
        address recipient;
        address primaryFeeTokenAddress;
        uint256 primaryFee;
        uint256 secondaryFee;
        uint256 requiredGasLimit;
        address multiHopFallback;
    }


    /// @notice Input parameters for transferring tokens to another chain as
    /// part of a transfer with a contract call.
    ///
    /// @param destinationBlockchainID BlockchainID of the destination
    ///
    /// @param destinationTokenTransferrerAddress Address of the destination
    /// token transferrer instance
    ///
    /// @param recipientContract The contract on the destination chain that
    /// will be called
    ///
    /// @param recipientPayload The payload that will be provided to the
    /// recipient contract on the destination chain
    ///
    /// @param requiredGasLimit The required amount of gas needed to deliver
    /// the message on its destination chain, including token operations and
    /// the call to the recipient contract.
    ///
    /// @param recipientGasLimit The amount of gas that will provided to the
    /// recipient contract on the destination chain, which must be less than
    /// the requiredGasLimit of the message as a whole.
    ///
    /// @param multiHopFallback In the case of a multi-hop transfer, the
    /// address where the tokens are sent on the home chain if the transfer is
    /// unable to be routed to its final destination. Note that this address
    /// must be able to receive the tokens held as collateral in the home
    /// contract.
    ///
    /// @param fallbackRecipient Address on the {destinationBlockchainID} where
    /// the transferred tokens are sent to if the call to the recipient
    /// contract fails. Note that this address must be able to receive the
    /// tokens on the destination chain of the transfer.
    ///
    /// @param primaryFeeTokenAddress Address of the ERC20 contract to
    /// optionally pay a Teleporter message fee
    ///
    /// @param primaryFee Amount of tokens to pay for Teleporter fee on the
    /// chain that iniiated the transfer
    ///
    /// @param secondaryFee Amount of tokens to pay for Teleporter fee if a
    /// multi-hop is needed
    struct SendAndCallInput {
        bytes32 destinationBlockchainID;
        address destinationTokenTransferrerAddress;
        address recipientContract;
        bytes recipientPayload;
        uint256 requiredGasLimit;
        uint256 recipientGasLimit;
        address multiHopFallback;
        address fallbackRecipient;
        address primaryFeeTokenAddress;
        uint256 primaryFee;
        uint256 secondaryFee;
    }

     /// @notice Interface for an Avalanche interchain token transferrer that
     /// sends tokens to another chain.
     ///
     /// @custom:security-contact
     /// https://github.com/ava-labs/icm-contracts/blob/main/SECURITY.md
    interface ITokenTransferrer is ITeleporterReceiver {
        /// @notice Emitted when tokens are sent to another chain.
        event TokensSent(
            bytes32 indexed teleporterMessageID,
            address indexed sender,
            SendTokensInput input,
            uint256 amount
        );

        /// @notice Emitted when tokens are sent to another chain with calldata
        /// for a contract recipient.
        event TokensAndCallSent(
            bytes32 indexed teleporterMessageID,
            address indexed sender,
            SendAndCallInput input,
            uint256 amount
        );

        /// @notice Emitted when tokens are withdrawn from the token transferrer
        /// contract.
        event TokensWithdrawn(address indexed recipient, uint256 amount);

        /// @notice Emitted when a call to a recipient contract to receive token
        /// succeeds.
        event CallSucceeded(address indexed recipientContract, uint256 amount);

        /// @notice Emitted when a call to a recipient contract to receive token
        /// fails, and the tokens are sent to a fallback recipient.
        event CallFailed(address indexed recipientContract, uint256 amount);
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
            // Make it an option for env config
            batch_size: 1000,
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

        let mut combined_stream = stream::empty::<(i64, DynProvider<Ethereum>, Vec<Log>)>().boxed();

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

            // Combine ICM (TeleporterMessenger) and ICTT (TokenTransferrer) event signatures
            let all_event_signatures: Vec<_> =
                ITeleporterMessenger::ITeleporterMessengerEvents::SIGNATURES
                    .iter()
                    .chain(ITokenTransferrer::ITokenTransferrerEvents::SIGNATURES.iter())
                    .copied()
                    .collect();

            let filter = Filter::new()
                // TODO: maybe we should still filter events by address for security reasons.
                // .address(vec![contract_address])
                .events(all_event_signatures);

            tracing::info!(bridge_id, chain_id, "Configured log stream");

            let stream_provider = provider.clone();
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
                .map(move |logs| (chain_id, stream_provider.clone(), logs))
                .boxed();

            combined_stream = stream::select(combined_stream, stream).boxed();
        }

        let native_id_to_chain_id = db
            .load_native_id_map()
            .await
            .context("failed to preload native blockchain id mapping")?;

        let buffer_handle = Arc::clone(&buffer).start().await?;

        // Process events
        while let Some((chain_id, provider, batch)) = combined_stream.next().await {
            match process_batch(
                batch,
                chain_id,
                bridge_id,
                &native_id_to_chain_id,
                &buffer,
                &provider,
            )
            .await
            {
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

/// Context extracted from logs within the same transaction.
/// Used to correlate events like CallSucceeded/CallFailed with their parent message.
#[derive(Debug, Default)]
struct TxContext {
    /// Message ID extracted from ReceiveCrossChainMessage in this transaction.
    /// Used to correlate destination-side ICTT events (CallSucceeded/CallFailed).
    message_id: Option<i64>,
    /// Source chain ID for the message (needed for Key construction)
    src_chain_id: Option<i64>,
}

async fn process_batch(
    batch: Vec<Log>,
    chain_id: i64,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer>,
    provider: &DynProvider<Ethereum>,
) -> Result<()> {
    // Group logs by transaction hash for same-tx correlation
    let mut logs_by_tx: HashMap<B256, Vec<&Log>> = HashMap::new();
    for log in &batch {
        if let Some(tx_hash) = log.transaction_hash {
            logs_by_tx.entry(tx_hash).or_default().push(log);
        }
    }

    // Process each transaction's logs together
    for (_tx_hash, tx_logs) in logs_by_tx {
        // First pass: extract context from ReceiveCrossChainMessage if present
        let mut tx_context = TxContext::default();
        for log in &tx_logs {
            if log.topic0() == Some(&ITeleporterMessenger::ReceiveCrossChainMessage::SIGNATURE_HASH)
                && let Ok(event) =
                    log.log_decode::<ITeleporterMessenger::ReceiveCrossChainMessage>()
            {
                let message_id_bytes = event.inner.messageID.as_slice();
                if let Ok(id_bytes) = message_id_bytes[..8].try_into() {
                    tx_context.message_id = Some(i64::from_be_bytes(id_bytes));
                }
                let source_hex = blockchain_id_hex(event.inner.sourceBlockchainID.as_slice());
                tx_context.src_chain_id = native_id_to_chain_id.get(&source_hex).copied();
            }
        }

        // Second pass: process all logs with context
        for log in tx_logs {
            let block_number = log.block_number.context("missing block number")? as i64;

            handle_log(
                chain_id,
                block_number,
                log,
                bridge_id,
                native_id_to_chain_id,
                buffer,
                &tx_context,
                provider,
            )
            .await?;
        }
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
    tx_context: &TxContext,
    provider: &DynProvider<Ethereum>,
) -> anyhow::Result<()> {
    match log.topic0() {
        // ICM Events
        Some(&ITeleporterMessenger::SendCrossChainMessage::SIGNATURE_HASH) => {
            handle_send_cross_chain_message(
                chain_id,
                block_number,
                log,
                bridge_id,
                native_id_to_chain_id,
                buffer,
                provider,
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
                provider,
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
        // ICTT Events
        Some(&ITokenTransferrer::TokensSent::SIGNATURE_HASH) => {
            handle_tokens_sent(chain_id, block_number, log, bridge_id, buffer).await
        }
        Some(&ITokenTransferrer::TokensAndCallSent::SIGNATURE_HASH) => {
            handle_tokens_and_call_sent(chain_id, block_number, log, bridge_id, buffer).await
        }
        Some(&ITokenTransferrer::TokensWithdrawn::SIGNATURE_HASH) => {
            handle_tokens_withdrawn(chain_id, block_number, log, bridge_id, buffer, tx_context)
                .await
        }
        Some(&ITokenTransferrer::CallSucceeded::SIGNATURE_HASH) => {
            handle_call_succeeded(chain_id, block_number, log, bridge_id, buffer, tx_context).await
        }
        Some(&ITokenTransferrer::CallFailed::SIGNATURE_HASH) => {
            handle_call_failed(chain_id, block_number, log, bridge_id, buffer, tx_context).await
        }
        _ => {
            tracing::trace!(
                topic0 = ?log.topic0(),
                "Ignoring unknown event"
            );
            Ok(())
        }
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
    provider: &DynProvider<Ethereum>,
) -> Result<()> {
    let event = log
        .log_decode::<ITeleporterMessenger::SendCrossChainMessage>()?
        .inner;
    let message_id_bytes = event.messageID.as_slice();
    let id = i64::from_be_bytes(message_id_bytes[..8].try_into()?);
    let tx_hash = log.transaction_hash.context("missing tx hash")?;

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

    // Fetch block timestamp via RPC (most nodes don't include it in logs)
    let block_timestamp = provider
        .get_block_by_number((block_number as u64).into())
        .await?
        .and_then(|block| {
            chrono::DateTime::from_timestamp(block.header.timestamp as i64, 0)
                .map(|dt| dt.naive_utc())
        });

    // Fill in source-side data
    entry.src_chain_id = Some(chain_id);
    entry.source_transaction_hash = Some(tx_hash);
    entry.init_timestamp = block_timestamp;
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
    provider: &DynProvider<Ethereum>,
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

    // Fetch block timestamp via RPC (most nodes don't include it in logs)
    let block_timestamp = provider
        .get_block_by_number((block_number as u64).into())
        .await?
        .and_then(|block| {
            chrono::DateTime::from_timestamp(block.header.timestamp as i64, 0)
                .map(|dt| dt.naive_utc())
        });

    // Fill in destination-side data
    entry.destination_chain_id = Some(chain_id);
    entry.destination_transaction_hash = Some(tx_hash.into());
    entry.native_id = Some(message_id_bytes.to_vec());
    entry.sender_address = event.message.originSenderAddress.into();
    entry.recipient_address = event.message.destinationAddress.into();
    entry.last_update_timestamp = block_timestamp;
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

// =============================================================================
// ICTT (Interchain Token Transfer) Handlers
// =============================================================================

/// Handle TokensSent - source-side ICTT event for simple token transfers
///
/// This event is emitted when tokens are sent to another chain.
/// The `teleporterMessageID` links this transfer to its parent ICM message.
///
/// Pushes an IcttEventFragment::TokensSent for later consolidation.
async fn handle_tokens_sent(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    buffer: &Arc<MessageBuffer>,
) -> Result<()> {
    let event = log.log_decode::<ITokenTransferrer::TokensSent>()?.inner;
    let message_id_bytes = event.teleporterMessageID.as_slice();
    let id = i64::from_be_bytes(message_id_bytes[..8].try_into()?);
    let token_contract = log.address();

    let key = Key::new(id, bridge_id);
    let mut entry = buffer.get_or_create(key).await?;

    // Push fragment for later consolidation
    entry.ictt_fragments.push(IcttEventFragment::TokensSent {
        token_contract,
        sender: event.sender,
        dst_token_address: event.input.destinationTokenTransferrerAddress,
        recipient: event.input.recipient,
        amount: event.amount,
    });

    entry.cursor.record_block(chain_id, block_number);
    buffer.upsert(entry).await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        sender = %event.sender,
        amount = %event.amount,
        "Processed TokensSent"
    );

    Ok(())
}

/// Handle TokensAndCallSent - source-side ICTT event for token transfers with contract call
///
/// Similar to TokensSent, but the recipient is a contract that will be called
/// with the provided payload after receiving the tokens.
///
/// Pushes an IcttEventFragment::TokensAndCallSent for later consolidation.
/// Stores fallbackRecipient so we can use it if CallFailed is received.
async fn handle_tokens_and_call_sent(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    buffer: &Arc<MessageBuffer>,
) -> Result<()> {
    let event = log
        .log_decode::<ITokenTransferrer::TokensAndCallSent>()?
        .inner;
    let message_id_bytes = event.teleporterMessageID.as_slice();
    let id = i64::from_be_bytes(message_id_bytes[..8].try_into()?);
    let token_contract = log.address();

    let key = Key::new(id, bridge_id);
    let mut entry = buffer.get_or_create(key).await?;

    // Push fragment for later consolidation
    entry
        .ictt_fragments
        .push(IcttEventFragment::TokensAndCallSent {
            token_contract,
            sender: event.sender,
            dst_token_address: event.input.destinationTokenTransferrerAddress,
            recipient_contract: event.input.recipientContract,
            fallback_recipient: event.input.fallbackRecipient,
            amount: event.amount,
        });

    entry.cursor.record_block(chain_id, block_number);
    buffer.upsert(entry).await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        sender = %event.sender,
        recipient_contract = %event.input.recipientContract,
        amount = %event.amount,
        "Processed TokensAndCallSent"
    );

    Ok(())
}

/// Handle TokensWithdrawn - destination-side ICTT event when tokens are released
///
/// This event is emitted when tokens are withdrawn to the recipient.
/// Uses TxContext to correlate with the parent message via messageID
/// extracted from ReceiveCrossChainMessage in the same transaction.
async fn handle_tokens_withdrawn(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    buffer: &Arc<MessageBuffer>,
    tx_context: &TxContext,
) -> Result<()> {
    let event = log
        .log_decode::<ITokenTransferrer::TokensWithdrawn>()?
        .inner;
    let tx_hash = log.transaction_hash.context("missing tx hash")?;

    // Use tx_context to find the parent message
    let Some(message_id) = tx_context.message_id else {
        tracing::debug!(
            recipient = %event.recipient,
            amount = %event.amount,
            tx_hash = %tx_hash,
            "TokensWithdrawn: no ReceiveCrossChainMessage in same tx, skipping"
        );
        return Ok(());
    };

    let key = Key::new(message_id, bridge_id);
    let mut entry = buffer.get_or_create(key).await?;

    // Push fragment for later consolidation
    entry
        .ictt_fragments
        .push(IcttEventFragment::TokensWithdrawn {
            recipient: event.recipient,
            amount: event.amount,
        });

    entry.cursor.record_block(chain_id, block_number);
    buffer.upsert(entry).await?;

    tracing::debug!(
        message_id,
        recipient = %event.recipient,
        amount = %event.amount,
        tx_hash = %tx_hash,
        chain_id,
        block_number,
        "Processed TokensWithdrawn"
    );

    Ok(())
}

/// Handle CallSucceeded - destination-side ICTT event when contract call succeeds
///
/// This event is emitted after TokensAndCallSent when the recipient contract
/// call succeeds. Uses TxContext to correlate with parent message via messageID
/// extracted from ReceiveCrossChainMessage in the same transaction.
///
/// Pushes an IcttEventFragment::CallSucceeded - consolidation will keep recipient
/// as the recipientContract from TokensAndCallSent.
async fn handle_call_succeeded(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    buffer: &Arc<MessageBuffer>,
    tx_context: &TxContext,
) -> Result<()> {
    let event = log.log_decode::<ITokenTransferrer::CallSucceeded>()?.inner;
    let tx_hash = log.transaction_hash.context("missing tx hash")?;

    // Use tx_context to find the parent message
    let Some(message_id) = tx_context.message_id else {
        tracing::debug!(
            recipient_contract = %event.recipientContract,
            amount = %event.amount,
            tx_hash = %tx_hash,
            "CallSucceeded: no ReceiveCrossChainMessage in same tx, skipping"
        );
        return Ok(());
    };

    let key = Key::new(message_id, bridge_id);
    let mut entry = buffer.get_or_create(key).await?;

    // Push fragment for later consolidation
    entry.ictt_fragments.push(IcttEventFragment::CallSucceeded {
        recipient_contract: event.recipientContract,
        amount: event.amount,
    });

    entry.cursor.record_block(chain_id, block_number);
    buffer.upsert(entry).await?;

    tracing::debug!(
        message_id,
        recipient_contract = %event.recipientContract,
        amount = %event.amount,
        tx_hash = %tx_hash,
        chain_id,
        block_number,
        "Processed CallSucceeded"
    );

    Ok(())
}

/// Handle CallFailed - destination-side ICTT event when contract call fails
///
/// This event is emitted after TokensAndCallSent when the recipient contract
/// call fails. Tokens are sent to the fallback recipient instead.
/// Uses TxContext to correlate with parent message via messageID
/// extracted from ReceiveCrossChainMessage in the same transaction.
///
/// Pushes an IcttEventFragment::CallFailed - consolidation will use
/// fallbackRecipient from TokensAndCallSent as the final recipient.
async fn handle_call_failed(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    buffer: &Arc<MessageBuffer>,
    tx_context: &TxContext,
) -> Result<()> {
    let event = log.log_decode::<ITokenTransferrer::CallFailed>()?.inner;
    let tx_hash = log.transaction_hash.context("missing tx hash")?;

    // Use tx_context to find the parent message
    let Some(message_id) = tx_context.message_id else {
        tracing::debug!(
            recipient_contract = %event.recipientContract,
            amount = %event.amount,
            tx_hash = %tx_hash,
            "CallFailed: no ReceiveCrossChainMessage in same tx, skipping"
        );
        return Ok(());
    };

    let key = Key::new(message_id, bridge_id);
    let mut entry = buffer.get_or_create(key).await?;

    // Push fragment for later consolidation - recipient will become fallbackRecipient
    entry.ictt_fragments.push(IcttEventFragment::CallFailed {
        recipient_contract: event.recipientContract,
        amount: event.amount,
    });

    entry.cursor.record_block(chain_id, block_number);
    buffer.upsert(entry).await?;

    tracing::debug!(
        message_id,
        recipient_contract = %event.recipientContract,
        amount = %event.amount,
        tx_hash = %tx_hash,
        chain_id,
        block_number,
        "Processed CallFailed"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_db;
    use alloy::{
        node_bindings::AnvilInstance,
        primitives::{Address, B256, Bytes, LogData, U256},
        providers::{ProviderBuilder, ext::AnvilApi},
        rpc::types::Log,
    };
    use interchain_indexer_entity::{crosschain_messages, sea_orm_active_enums::MessageStatus};
    use sea_orm::{ActiveValue, EntityTrait};

    /// Create a mock provider for tests using an embedded Anvil instance.
    /// Mines 300 blocks so that tests can reference blocks 0-299.
    async fn create_test_provider() -> (DynProvider<Ethereum>, AnvilInstance) {
        let anvil = alloy::node_bindings::Anvil::new().spawn();
        let provider = ProviderBuilder::new()
            .connect_http(anvil.endpoint_url())
            .erased();

        // Mine 300 blocks so tests can reference block numbers 100, 200, 201, etc.
        for _ in 0..300 {
            provider.evm_mine(None).await.unwrap();
        }

        (provider, anvil)
    }

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
        DynProvider<Ethereum>,
        AnvilInstance, // keep anvil alive for the duration of the test
        i32,           // bridge_id
        i64,           // src_chain_id
        i64,           // dst_chain_id
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

        let (provider, anvil) = create_test_provider().await;

        (
            interchain_db,
            buffer,
            native_id_to_chain_id,
            provider,
            anvil,
            bridge_id,
            src_chain_id,
            dst_chain_id,
        )
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_sequential_event_flow_with_buffer() {
        let (
            interchain_db,
            buffer,
            native_id_to_chain_id,
            provider,
            _anvil,
            bridge_id,
            src_chain_id,
            dst_chain_id,
        ) = setup_test_env("avax_buf_seq").await;

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
            &provider,
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
            &provider,
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
        let (
            interchain_db,
            buffer,
            native_id_to_chain_id,
            provider,
            _anvil,
            bridge_id,
            src_chain_id,
            dst_chain_id,
        ) = setup_test_env("avax_buf_ooo").await;

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
            &provider,
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
            &provider,
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
        let (
            interchain_db,
            buffer,
            native_id_to_chain_id,
            provider,
            _anvil,
            bridge_id,
            src_chain_id,
            dst_chain_id,
        ) = setup_test_env("avax_buf_exec_first").await;

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
            &provider,
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
        let (
            interchain_db,
            buffer,
            native_id_to_chain_id,
            provider,
            _anvil,
            bridge_id,
            src_chain_id,
            dst_chain_id,
        ) = setup_test_env("avax_buf_failed").await;

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
            &provider,
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
            &provider,
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
