pub mod abi;
pub mod consolidation;
pub mod types;

use alloy::{
    network::Ethereum,
    primitives::{Address, B256},
    providers::{DynProvider, Provider as _},
    rpc::types::{Filter, Log},
    sol_types::SolEvent,
};
use anyhow::{Context, Result, anyhow};
use futures::{StreamExt, stream};

use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::task::JoinHandle;

use crate::{
    InterchainDatabase,
    log_stream::LogStreamBuilder,
    message_buffer::{Config, Key, MessageBuffer},
};

use abi::{ITeleporterMessenger, ITokenHome, ITokenTransferrer};

use types::{
    AnnotatedEvent, AnnotatedICTTSource, CallOutcome, Message, MessageExecutionOutcome,
    SentOrRouted, SentOrRoutedAndCalled, TokenTransfer,
};

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
    buffer: Arc<MessageBuffer<Message>>,
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

            let filter = Filter::new()
                .address(contract_address)
                .events(ITeleporterMessenger::ITeleporterMessengerEvents::SIGNATURES);

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

async fn process_batch(
    batch: Vec<Log>,
    chain_id: i64,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer<Message>>,
    provider: &DynProvider<Ethereum>,
) -> Result<()> {
    // Group logs by transaction hash for same-tx correlation
    let mut logs_by_transaction_hash: HashMap<B256, Vec<&Log>> = HashMap::new();

    for log in &batch {
        if let Some(transaction_hash) = log.transaction_hash {
            logs_by_transaction_hash
                .entry(transaction_hash)
                .or_default()
                .push(log);
        }
    }

    // Process each transaction's logs together
    for (_hash, logs) in logs_by_transaction_hash {
        // Process all logs in this transaction.
        for log in logs {
            let block_number = log.block_number.context("missing block number")? as i64;

            handle_log(
                chain_id,
                block_number,
                log,
                bridge_id,
                native_id_to_chain_id,
                buffer,
                provider,
            )
            .await?;
        }
    }

    Ok(())
}

/// Fetch block timestamp from RPC
async fn fetch_block_timestamp(
    provider: &DynProvider<Ethereum>,
    block_number: i64,
) -> Result<chrono::NaiveDateTime> {
    let block = provider
        .get_block_by_number((block_number as u64).into())
        .await?
        .context("block not found")?;

    chrono::DateTime::from_timestamp(block.header.timestamp as i64, 0)
        .map(|dt| dt.naive_utc())
        .context("invalid timestamp")
}

use std::collections::HashSet;

/// Fetch ICTT logs from the same transaction
///
/// TODO: use eth_getLogs with filter by transaction hash Also there's
/// ITokenTransferrer::ITokenTransferrerEvents::SIGNATURES for use in filter.
async fn fetch_ictt_logs_for_transaction(
    provider: &DynProvider<Ethereum>,
    transaction_hash: B256,
) -> Result<Vec<Log>> {
    let receipt = provider
        .get_transaction_receipt(transaction_hash)
        .await?
        .context("transaction receipt not found")?;

    let ictt_topics: HashSet<B256> = ITokenTransferrer::ITokenTransferrerEvents::SELECTORS
        .iter()
        .chain(ITokenHome::ITokenHomeEvents::SELECTORS)
        .map(|selector| B256::from_slice(selector.as_slice()))
        .collect();

    let logs: Vec<Log> = receipt
        .inner
        .logs()
        .iter()
        .filter(|log| {
            log.topic0()
                .map(|t| ictt_topics.contains(t))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    Ok(logs)
}

/// Parse ICTT logs into TokenTransfer. Returns None if no ICTT logs found.
///
/// TODO: We should validate that our invariant about one log correlation holds.
/// If not, we should throw error. Also use Result<Option<TokenTransfer>> I guess?
fn parse_ictt_logs(logs: &[Log]) -> Option<TokenTransfer> {
    for log in logs {
        let contract_address = log.address();

        if log.topic0() == Some(&ITokenTransferrer::TokensSent::SIGNATURE_HASH) {
            if let Ok(event) = log.log_decode::<ITokenTransferrer::TokensSent>() {
                let sent = SentOrRouted::Sent(AnnotatedICTTSource {
                    event: event.inner.data.clone(),
                    contract_address,
                });
                // Look for corresponding TokensWithdrawn
                let withdrawn = logs.iter().find_map(|l| {
                    if l.topic0() == Some(&ITokenTransferrer::TokensWithdrawn::SIGNATURE_HASH) {
                        l.log_decode::<ITokenTransferrer::TokensWithdrawn>()
                            .ok()
                            .map(|e| e.inner.data.clone())
                    } else {
                        None
                    }
                });
                return Some(TokenTransfer::Sent(Some(sent), withdrawn));
            }
        }

        // TokensAndCallSent source event
        if log.topic0() == Some(&ITokenTransferrer::TokensAndCallSent::SIGNATURE_HASH) {
            if let Ok(event) = log.log_decode::<ITokenTransferrer::TokensAndCallSent>() {
                let sent = SentOrRoutedAndCalled::Sent(AnnotatedICTTSource {
                    event: event.inner.data.clone(),
                    contract_address,
                });
                // Look for CallSucceeded or CallFailed
                let outcome = logs.iter().find_map(|l| {
                    if l.topic0() == Some(&ITokenTransferrer::CallSucceeded::SIGNATURE_HASH) {
                        l.log_decode::<ITokenTransferrer::CallSucceeded>()
                            .ok()
                            .map(|e| CallOutcome::Succeeded(e.inner.data.clone()))
                    } else if l.topic0() == Some(&ITokenTransferrer::CallFailed::SIGNATURE_HASH) {
                        l.log_decode::<ITokenTransferrer::CallFailed>()
                            .ok()
                            .map(|e| CallOutcome::Failed(e.inner.data.clone()))
                    } else {
                        None
                    }
                });
                return Some(TokenTransfer::SentAndCalled(Some(sent), outcome));
            }
        }

        // TokensRouted (multi-hop) source event
        if log.topic0() == Some(&ITokenHome::TokensRouted::SIGNATURE_HASH) {
            if let Ok(event) = log.log_decode::<ITokenHome::TokensRouted>() {
                let routed = SentOrRouted::Routed(AnnotatedICTTSource {
                    event: event.inner.data.clone(),
                    contract_address,
                });
                let withdrawn = logs.iter().find_map(|l| {
                    if l.topic0() == Some(&ITokenTransferrer::TokensWithdrawn::SIGNATURE_HASH) {
                        l.log_decode::<ITokenTransferrer::TokensWithdrawn>()
                            .ok()
                            .map(|e| e.inner.data.clone())
                    } else {
                        None
                    }
                });
                return Some(TokenTransfer::Sent(Some(routed), withdrawn));
            }
        }

        // TokensAndCallRouted (multi-hop) source event
        if log.topic0() == Some(&ITokenHome::TokensAndCallRouted::SIGNATURE_HASH) {
            if let Ok(event) = log.log_decode::<ITokenHome::TokensAndCallRouted>() {
                let routed = SentOrRoutedAndCalled::Routed(AnnotatedICTTSource {
                    event: event.inner.data.clone(),
                    contract_address,
                });
                let outcome = logs.iter().find_map(|l| {
                    if l.topic0() == Some(&ITokenTransferrer::CallSucceeded::SIGNATURE_HASH) {
                        l.log_decode::<ITokenTransferrer::CallSucceeded>()
                            .ok()
                            .map(|e| CallOutcome::Succeeded(e.inner.data.clone()))
                    } else if l.topic0() == Some(&ITokenTransferrer::CallFailed::SIGNATURE_HASH) {
                        l.log_decode::<ITokenTransferrer::CallFailed>()
                            .ok()
                            .map(|e| CallOutcome::Failed(e.inner.data.clone()))
                    } else {
                        None
                    }
                });
                return Some(TokenTransfer::SentAndCalled(Some(routed), outcome));
            }
        }
    }

    // No source-side ICTT events found - check for destination-only events
    // (This happens when we receive on destination but haven't seen send yet)
    for log in logs {
        if log.topic0() == Some(&ITokenTransferrer::TokensWithdrawn::SIGNATURE_HASH) {
            if let Ok(event) = log.log_decode::<ITokenTransferrer::TokensWithdrawn>() {
                return Some(TokenTransfer::Sent(None, Some(event.inner.data.clone())));
            }
        }
        if log.topic0() == Some(&ITokenTransferrer::CallSucceeded::SIGNATURE_HASH) {
            if let Ok(event) = log.log_decode::<ITokenTransferrer::CallSucceeded>() {
                return Some(TokenTransfer::SentAndCalled(
                    None,
                    Some(CallOutcome::Succeeded(event.inner.data.clone())),
                ));
            }
        }
        if log.topic0() == Some(&ITokenTransferrer::CallFailed::SIGNATURE_HASH) {
            if let Ok(event) = log.log_decode::<ITokenTransferrer::CallFailed>() {
                return Some(TokenTransfer::SentAndCalled(
                    None,
                    Some(CallOutcome::Failed(event.inner.data.clone())),
                ));
            }
        }
    }

    None
}

/// Handle ICM events - only handles Send/Receive/Execute events
/// ICTT logs are fetched on-demand when processing Receive events
async fn handle_log(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer<Message>>,
    provider: &DynProvider<Ethereum>,
) -> anyhow::Result<()> {
    match log.topic0() {
        // ICM Source Event
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
        // ICM Destination Events - ReceiveCrossChainMessage may have execution in same tx
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
        // Execution events - handled for retry case (retryMessageExecution)
        // When execution comes via retry, it's in a different transaction than ReceiveCrossChainMessage
        Some(&ITeleporterMessenger::MessageExecuted::SIGNATURE_HASH) => {
            handle_message_executed(
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
        Some(&ITeleporterMessenger::MessageExecutionFailed::SIGNATURE_HASH) => {
            handle_message_execution_failed(
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
        _ => {
            tracing::trace!(
                topic0 = ?log.topic0(),
                "Ignoring unknown event"
            );
            Ok(())
        }
    }
}

/// Handle SendCrossChainMessage - source-side event
/// Fetches ICTT logs from the same transaction to capture TokensSent/TokensAndCallSent
async fn handle_send_cross_chain_message(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer<Message>>,
    provider: &DynProvider<Ethereum>,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::SendCrossChainMessage>()?;
    let event = decoded.inner.data.clone();
    let message_id_bytes: [u8; 8] = event.messageID.as_slice()[..8].try_into()?;
    let id = i64::from_be_bytes(message_id_bytes);
    let transaction_hash = log.transaction_hash.context("missing transaction hash")?;

    let destination_hex = blockchain_id_hex(event.destinationBlockchainID.as_slice());
    let dst_chain_id = native_id_to_chain_id.get(&destination_hex).copied();

    // Skip messages to untracked chains
    if dst_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            destination_blockchain_id = %destination_hex,
            "Skipping SendCrossChainMessage to untracked chain"
        );
        return Ok(());
    }

    let key = Key::new(id, bridge_id);

    // Fetch block timestamp and ICTT logs in parallel
    let (block_timestamp, ictt_logs) = tokio::try_join!(
        fetch_block_timestamp(provider, block_number),
        fetch_ictt_logs_for_transaction(provider, transaction_hash)
    )?;

    // Parse ICTT logs
    let transfer = parse_ictt_logs(&ictt_logs);

    // Build AnnotatedEvent
    let annotated_send = AnnotatedEvent {
        event,
        transaction_hash,
        block_number,
        block_timestamp,
        chain_id,
    };

    // Get or create message entry
    let mut entry = buffer.get_or_default(key).await?;

    // Update send field (source-side)
    entry.inner.send = Some(annotated_send);

    // Merge ICTT transfer data - keep existing destination-side data if present
    if let Some(new_transfer) = transfer {
        entry.inner.transfer = match (entry.inner.transfer.take(), new_transfer) {
            (None, new) => Some(new),
            (Some(TokenTransfer::Sent(_, existing_dst)), TokenTransfer::Sent(src, _)) => {
                Some(TokenTransfer::Sent(src, existing_dst))
            }
            (
                Some(TokenTransfer::SentAndCalled(_, existing_dst)),
                TokenTransfer::SentAndCalled(src, _),
            ) => Some(TokenTransfer::SentAndCalled(src, existing_dst)),
            (existing, _) => existing, // Keep existing if types don't match
        };
    }

    buffer
        .upsert_with_cursors(key, entry, chain_id, block_number)
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        "Processed SendCrossChainMessage"
    );

    Ok(())
}

/// Handle ReceiveCrossChainMessage - destination-side event
/// Also fetches MessageExecuted/MessageExecutionFailed and ICTT events from same tx
async fn handle_receive_cross_chain_message(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer<Message>>,
    provider: &DynProvider<Ethereum>,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::ReceiveCrossChainMessage>()?;
    let event = decoded.inner.data.clone();
    let message_id_bytes: [u8; 8] = event.messageID.as_slice()[..8].try_into()?;
    let id = i64::from_be_bytes(message_id_bytes);
    let transaction_hash = log.transaction_hash.context("missing transaction hash")?;

    let source_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());
    let src_chain_id = native_id_to_chain_id.get(&source_hex).copied();

    // Skip messages from untracked chains
    if src_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_hex,
            "Skipping ReceiveCrossChainMessage from untracked chain"
        );
        return Ok(());
    }

    let key = Key::new(id, bridge_id);

    // Fetch block timestamp
    let block_timestamp = fetch_block_timestamp(provider, block_number).await?;

    // Fetch all logs from this transaction to find execution outcome and ICTT events
    let receipt = provider
        .get_transaction_receipt(transaction_hash)
        .await?
        .context("transaction receipt not found")?;
    let transaction_logs: Vec<_> = receipt.inner.logs().to_vec();

    // Find execution outcome (MessageExecuted or MessageExecutionFailed) in same transaction
    let execution_outcome = transaction_logs.iter().find_map(|l| {
        if l.topic0() == Some(&ITeleporterMessenger::MessageExecuted::SIGNATURE_HASH) {
            l.log_decode::<ITeleporterMessenger::MessageExecuted>()
                .ok()
                .filter(|e| e.inner.data.messageID == event.messageID)
                .map(|e| {
                    MessageExecutionOutcome::Succeeded(AnnotatedEvent {
                        event: e.inner.data.clone(),
                        transaction_hash,
                        block_number,
                        block_timestamp,
                        chain_id,
                    })
                })
        } else if l.topic0() == Some(&ITeleporterMessenger::MessageExecutionFailed::SIGNATURE_HASH)
        {
            l.log_decode::<ITeleporterMessenger::MessageExecutionFailed>()
                .ok()
                .filter(|e| e.inner.data.messageID == event.messageID)
                .map(|e| {
                    MessageExecutionOutcome::Failed(AnnotatedEvent {
                        event: e.inner.data.clone(),
                        transaction_hash,
                        block_number,
                        block_timestamp,
                        chain_id,
                    })
                })
        } else {
            None
        }
    });

    // Parse ICTT logs from same transaction (only present if execution succeeded)
    let transfer = parse_ictt_logs(&transaction_logs);

    // Build AnnotatedEvent for receive
    let annotated_receive = AnnotatedEvent {
        event,
        transaction_hash,
        block_number,
        block_timestamp,
        chain_id,
    };

    // Get or create message entry
    let mut entry = buffer.get_or_default(key).await?;

    // Update receive field
    entry.inner.receive = Some(annotated_receive);

    // Update execution field if we found one in the same transaction
    if let Some(outcome) = execution_outcome {
        entry.inner.execution = Some(outcome);
    }

    // Merge ICTT transfer data - keep existing source-side data if present
    if let Some(new_transfer) = transfer {
        entry.inner.transfer = match (entry.inner.transfer.take(), new_transfer) {
            (None, new) => Some(new),
            (Some(TokenTransfer::Sent(existing_src, _)), TokenTransfer::Sent(_, dst)) => {
                Some(TokenTransfer::Sent(existing_src, dst))
            }
            (
                Some(TokenTransfer::SentAndCalled(existing_src, _)),
                TokenTransfer::SentAndCalled(_, dst),
            ) => Some(TokenTransfer::SentAndCalled(existing_src, dst)),
            (existing, _) => existing, // Keep existing if types don't match
        };
    }

    buffer
        .upsert_with_cursors(key, entry, chain_id, block_number)
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        "Processed ReceiveCrossChainMessage"
    );

    Ok(())
}

/// Handle MessageExecuted - can come via retry (retryMessageExecution)
/// This handles the case where execution happens in a separate transaction from receive
async fn handle_message_executed(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer<Message>>,
    provider: &DynProvider<Ethereum>,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::MessageExecuted>()?;
    let event = decoded.inner.data.clone();
    let message_id_bytes: [u8; 8] = event.messageID.as_slice()[..8].try_into()?;
    let id = i64::from_be_bytes(message_id_bytes);
    let transaction_hash = log.transaction_hash.context("missing transaction hash")?;

    let source_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());
    let src_chain_id = native_id_to_chain_id.get(&source_hex).copied();

    // Skip messages from untracked chains
    if src_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_hex,
            "Skipping MessageExecuted from untracked chain"
        );
        return Ok(());
    }

    let key = Key::new(id, bridge_id);

    // Fetch block timestamp
    let block_timestamp = fetch_block_timestamp(provider, block_number).await?;

    // Fetch ICTT logs from this transaction (retry execution may have TokensWithdrawn, etc.)
    let ictt_logs = fetch_ictt_logs_for_transaction(provider, transaction_hash).await?;
    let transfer = parse_ictt_logs(&ictt_logs);

    // Build AnnotatedEvent for execution
    let annotated_execution = AnnotatedEvent {
        event,
        transaction_hash,
        block_number,
        block_timestamp,
        chain_id,
    };

    // Get or create message entry
    let mut entry = buffer.get_or_default(key).await?;

    // Update execution field - this replaces any previous Failed execution with Succeeded
    entry.inner.execution = Some(MessageExecutionOutcome::Succeeded(annotated_execution));

    // Merge ICTT transfer data - keep existing source-side data if present
    if let Some(new_transfer) = transfer {
        entry.inner.transfer = match (entry.inner.transfer.take(), new_transfer) {
            (None, new) => Some(new),
            (Some(TokenTransfer::Sent(existing_src, _)), TokenTransfer::Sent(_, dst)) => {
                Some(TokenTransfer::Sent(existing_src, dst))
            }
            (
                Some(TokenTransfer::SentAndCalled(existing_src, _)),
                TokenTransfer::SentAndCalled(_, dst),
            ) => Some(TokenTransfer::SentAndCalled(existing_src, dst)),
            (existing, _) => existing,
        };
    }

    buffer
        .upsert_with_cursors(key, entry, chain_id, block_number)
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        "Processed MessageExecuted (retry)"
    );

    Ok(())
}

/// Handle MessageExecutionFailed - execution failed, can be retried later
async fn handle_message_execution_failed(
    chain_id: i64,
    block_number: i64,
    log: &Log,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer<Message>>,
    provider: &DynProvider<Ethereum>,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::MessageExecutionFailed>()?;
    let event = decoded.inner.data.clone();
    let message_id_bytes: [u8; 8] = event.messageID.as_slice()[..8].try_into()?;
    let id = i64::from_be_bytes(message_id_bytes);
    let transaction_hash = log.transaction_hash.context("missing transaction hash")?;

    let source_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());
    let src_chain_id = native_id_to_chain_id.get(&source_hex).copied();

    // Skip messages from untracked chains
    if src_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_hex,
            "Skipping MessageExecutionFailed from untracked chain"
        );
        return Ok(());
    }

    let key = Key::new(id, bridge_id);

    // Fetch block timestamp
    let block_timestamp = fetch_block_timestamp(provider, block_number).await?;

    // Build AnnotatedEvent for execution failure
    let annotated_execution = AnnotatedEvent {
        event,
        transaction_hash,
        block_number,
        block_timestamp,
        chain_id,
    };

    // Get or create message entry
    let mut entry = buffer.get_or_default(key).await?;

    // Only update if not already succeeded (don't overwrite success with failure)
    if !matches!(
        entry.inner.execution,
        Some(MessageExecutionOutcome::Succeeded(_))
    ) {
        entry.inner.execution = Some(MessageExecutionOutcome::Failed(annotated_execution));
    }

    buffer
        .upsert_with_cursors(key, entry, chain_id, block_number)
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        "Processed MessageExecutionFailed"
    );

    Ok(())
}
