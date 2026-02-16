//! Avalanche Teleporter (ICM) + ICTT indexer implementation.
//!
//! ## Summary
//! - Streams Teleporter logs across multiple Avalanche L1 chains.
//! - Groups logs per transaction to preserve execution context.
//! - Resolves blockchain IDs to EVM chain IDs and filters unknown chains.
//! - Builds per-message state incrementally in the message buffer.
//! - Persists finalized records via consolidation rules.
//!
//! Detailed semantics live on the individual handlers and helpers below.
//!
//! ## See also
//! - [All possible ICTT + ICM
//!   flows](https://blockscout.notion.site/All-possible-ICTT-ICM-flows-2c53d73641f88060b550e882752968ad)
pub mod abi;
mod blockchain_id_resolver;
pub mod consolidation;
pub mod settings;
pub mod types;

use alloy::{
    hex,
    network::Ethereum,
    primitives::{Address, B256},
    providers::{DynProvider, Provider as _},
    rpc::types::{Block, Filter, Log},
    sol_types::SolEvent,
};
use anyhow::{Context, Error, Result, anyhow};
use futures::{StreamExt, TryStreamExt, stream, stream::SelectAll};
use itertools::Itertools;
use std::sync::atomic::Ordering;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::task::JoinHandle;
use tonic::async_trait;

use crate::{
    CrosschainIndexer, CrosschainIndexerState, CrosschainIndexerStatus, InterchainDatabase,
    avalanche::settings::AvalancheIndexerSettings,
    log_stream::LogStream,
    message_buffer::{Config, Key, MessageBuffer},
};

use abi::{ITeleporterMessenger, ITokenHome, ITokenTransferrer};
use blockchain_id_resolver::{AvalancheDataApiNetwork, BlockchainIdResolver};

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
    /// If true, do not drop ICM events whose resolved EVM chain id is not present in
    /// `tracked_chain_ids`.
    ///
    /// Chains without an EVM chain id (resolver returns `None`) are still skipped.
    pub process_unknown_chains: bool,
}

impl AvalancheIndexerConfig {
    pub fn new(
        bridge_id: i32,
        chains: Vec<AvalancheChainConfig>,
        settings: &AvalancheIndexerSettings,
    ) -> Self {
        Self {
            bridge_id,
            chains,
            poll_interval: settings.pull_interval_ms,
            batch_size: settings.batch_size,
            process_unknown_chains: settings.process_unknown_chains,
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

pub struct AvalancheIndexer {
    db: Arc<InterchainDatabase>,
    config: AvalancheIndexerConfig,
    buffer: Arc<MessageBuffer<Message>>,
    buffer_handle: Arc<parking_lot::RwLock<Option<JoinHandle<()>>>>,

    is_running: Arc<std::sync::atomic::AtomicBool>,
    indexing_handle: Arc<parking_lot::RwLock<Option<JoinHandle<()>>>>,
    state: Arc<parking_lot::RwLock<CrosschainIndexerState>>,
    init_timestamp: chrono::NaiveDateTime,
    error_count: Arc<std::sync::atomic::AtomicU64>,
}

/// Cleanup guard that ensures proper cleanup when the indexer task exits.
/// On drop, it clears handles, aborts the buffer task, and sets the appropriate state.
struct IndexerCleanupGuard {
    is_running: Arc<std::sync::atomic::AtomicBool>,
    state: Arc<parking_lot::RwLock<CrosschainIndexerState>>,
    buffer_handle: Arc<parking_lot::RwLock<Option<JoinHandle<()>>>>,
    indexing_handle: Arc<parking_lot::RwLock<Option<JoinHandle<()>>>>,
    bridge_id: i32,
}

impl Drop for IndexerCleanupGuard {
    fn drop(&mut self) {
        tracing::debug!(
            bridge_id = self.bridge_id,
            "Indexer cleanup guard triggered"
        );

        // Mark as not running
        self.is_running.store(false, Ordering::Release);

        // Abort and clear the buffer handle
        if let Some(handle) = self.buffer_handle.write().take() {
            handle.abort();
        }

        // Clear the indexing handle (don't abort - we're inside it)
        let _ = self.indexing_handle.write().take();

        // Set final state based on whether there was an error
        // Note: if an error occurred, the state was already set to Failed in the task
        // This handles the case of clean exit or abort
        let current_state = self.state.read().clone();
        if !matches!(current_state, CrosschainIndexerState::Failed(_)) {
            *self.state.write() = CrosschainIndexerState::Idle;
        }
    }
}

impl AvalancheIndexer {
    pub fn new(db: Arc<InterchainDatabase>, config: AvalancheIndexerConfig) -> Result<Self> {
        if config.chains.is_empty() {
            return Err(anyhow!(
                "Avalanche indexer requires at least one configured chain"
            ));
        }

        let buffer_config = Config::default();
        let buffer = MessageBuffer::new((*db).clone(), buffer_config);

        Ok(Self {
            db,
            config,
            buffer,
            buffer_handle: Arc::new(parking_lot::RwLock::new(None)),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            indexing_handle: Arc::new(parking_lot::RwLock::new(None)),
            state: Arc::new(parking_lot::RwLock::new(CrosschainIndexerState::Idle)),
            init_timestamp: chrono::Utc::now().naive_utc(),
            error_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }

    fn clone_for_task(&self) -> Self {
        Self {
            db: self.db.clone(),
            config: self.config.clone(),
            buffer: self.buffer.clone(),
            buffer_handle: self.buffer_handle.clone(),
            is_running: self.is_running.clone(),
            indexing_handle: self.indexing_handle.clone(),
            state: self.state.clone(),
            init_timestamp: self.init_timestamp,
            error_count: self.error_count.clone(),
        }
    }

    /// Main indexing loop.
    ///
    /// - Restores cursors from checkpoints (or starts from config).
    /// - Builds one log stream per chain (catchup + realtime).
    /// - Merges streams and processes batches in order of arrival.
    async fn run(self) -> Result<()> {
        let db = (*self.db).clone();
        let config = self.config;
        let buffer = self.buffer;
        let AvalancheIndexerConfig {
            bridge_id,
            chains,
            poll_interval,
            batch_size,
            process_unknown_chains,
        } = config;

        let chain_ids: HashSet<i64> = chains.iter().map(|c| c.chain_id).collect();

        let data_api_network = std::env::var("AVALANCHE_DATA_API_NETWORK")
            .ok()
            .and_then(|v| match v.to_ascii_lowercase().as_str() {
                "mainnet" => Some(AvalancheDataApiNetwork::Mainnet),
                "fuji" => Some(AvalancheDataApiNetwork::Fuji),
                "testnet" => Some(AvalancheDataApiNetwork::Testnet),
                _ => None,
            })
            .unwrap_or(AvalancheDataApiNetwork::Mainnet);

        let data_api_key = std::env::var("AVALANCHE_GLACIER_API_KEY")
            .ok()
            .or_else(|| std::env::var("AVALANCHE_DATA_API_KEY").ok());

        let blockchain_id_resolver =
            BlockchainIdResolver::new(data_api_network, data_api_key, db.clone());

        tracing::info!(
            bridge_id,
            chain_count = chains.len(),
            "starting Avalanche indexer"
        );

        let mut combined_stream = SelectAll::new();

        for chain in chains {
            let chain_id = chain.chain_id;
            let start_block = chain.start_block;
            let contract_address = chain.contract_address;
            let provider = chain.provider.clone();

            // Restore checkpoint if it exists for this bridge and chain.
            let checkpoint = db.get_checkpoint(bridge_id as u64, chain_id as u64).await?;

            let (realtime_cursor, catchup_cursor) = if let Some(cp) = checkpoint {
                let realtime_cursor = cp.validated_realtime_cursor();
                let catchup_cursor = cp.validated_catchup_cursor();

                tracing::info!(
                    bridge_id,
                    chain_id,
                    realtime_cursor,
                    catchup_cursor,
                    "restored Avalanche indexer checkpoint"
                );

                (realtime_cursor, catchup_cursor)
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

            tracing::info!(bridge_id, chain_id, "configured log stream");

            let stream_provider = provider.clone();
            let stream = LogStream::builder(provider.clone())
                .filter(filter)
                .poll_interval(poll_interval)
                .batch_size(batch_size)
                .genesis_block(start_block)
                .realtime_cursor(realtime_cursor)
                .catchup_cursor(catchup_cursor)
                .bridge_id(bridge_id)
                .chain_id(chain_id)
                .catchup()
                .realtime()
                .build()?
                .map(move |logs| (chain_id, stream_provider.clone(), logs))
                .boxed();

            combined_stream.push(stream);
        }

        let batch_ctx = BatchProcessContext {
            bridge_id,
            chain_ids: &chain_ids,
            process_unknown_chains,
            blockchain_id_resolver: &blockchain_id_resolver,
            buffer: &buffer,
        };

        // Process events
        while let Some((chain_id, provider, batch)) = combined_stream.next().await {
            match process_batch(batch, chain_id, &batch_ctx, &provider).await {
                Ok(_) => {
                    tracing::debug!(bridge_id, chain_id, "processed log batch");
                }
                Err(err) => {
                    tracing::error!(
                        err = ?err,
                        bridge_id,
                        chain_id,
                        "failed to process Avalanche log batch"
                    );
                }
            }
        }

        tracing::warn!(bridge_id, "Avalanche indexer stream completed unexpectedly");
        Ok(())
    }
}

#[async_trait]
impl CrosschainIndexer for AvalancheIndexer {
    fn name(&self) -> String {
        "avalanche".to_string()
    }

    fn description(&self) -> String {
        "Avalanche Teleporter (ICM) + ICTT indexer".to_string()
    }

    async fn start(&self) -> Result<(), Error> {
        // Atomic compare-and-set: only one caller can proceed
        if self
            .is_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            tracing::warn!(
                bridge_id = self.config.bridge_id,
                "Avalanche indexer already running"
            );
            return Ok(());
        }

        // Start the buffer task
        let buffer_task_handle = match Arc::clone(&self.buffer).start().await {
            Ok(handle) => handle,
            Err(err) => {
                // Rollback is_running on failure
                self.is_running.store(false, Ordering::Release);
                return Err(err);
            }
        };
        *self.buffer_handle.write() = Some(buffer_task_handle);

        *self.state.write() = CrosschainIndexerState::Running;

        let this = self.clone_for_task();
        let bridge_id = this.config.bridge_id;

        let handle = tokio::spawn(async move {
            // Extract Arc references before moving `this` into run()
            let is_running = this.is_running.clone();
            let state = this.state.clone();
            let buffer_handle = this.buffer_handle.clone();
            let indexing_handle = this.indexing_handle.clone();
            let error_count = this.error_count.clone();

            // Cleanup guard that runs on drop (whether success, error, or abort)
            let _cleanup_guard = IndexerCleanupGuard {
                is_running: is_running.clone(),
                state: state.clone(),
                buffer_handle,
                indexing_handle,
                bridge_id,
            };

            if !is_running.load(Ordering::Acquire) {
                return;
            }

            if let Err(err) = this.run().await {
                error_count.fetch_add(1, Ordering::Relaxed);
                tracing::error!(err = ?err, bridge_id, "Avalanche indexer task stopped with error");
                *state.write() = CrosschainIndexerState::Failed(format!("{err:#}"));
            }
            // On clean exit without error, the guard will set state to Idle
        });

        *self.indexing_handle.write() = Some(handle);
        Ok(())
    }

    async fn stop(&self) {
        self.is_running.store(false, Ordering::Release);
        if let Some(handle) = self.indexing_handle.write().take() {
            handle.abort();
        }
        if let Some(handle) = self.buffer_handle.write().take() {
            handle.abort();
        }
        *self.state.write() = CrosschainIndexerState::Idle;
    }

    fn get_state(&self) -> CrosschainIndexerState {
        self.state.read().clone()
    }

    fn get_status(&self) -> CrosschainIndexerStatus {
        CrosschainIndexerStatus {
            state: self.state.read().clone(),
            init_timestamp: self.init_timestamp,
            extra_info: std::collections::HashMap::from([
                (
                    "chains_count".to_string(),
                    serde_json::json!(self.config.chains.len()),
                ),
                (
                    "poll_interval_secs".to_string(),
                    serde_json::json!(self.config.poll_interval.as_secs()),
                ),
                (
                    "batch_size".to_string(),
                    serde_json::json!(self.config.batch_size),
                ),
            ]),
        }
    }
}

/// Extract the indexer message key from a Teleporter `messageID`.
///
/// Current mapping uses the first 8 bytes as a big-endian `i64`, combined with
/// `bridge_id`. This is a convention, not a protocol requirement.
fn parse_message_key(message_id: &B256, bridge_id: i32) -> Result<(Key, [u8; 8])> {
    let message_id_bytes: [u8; 8] = message_id.as_slice()[..8].try_into()?;
    let id = i64::from_be_bytes(message_id_bytes);
    let bridge_id = i16::try_from(bridge_id).context("bridge_id out of range")?;
    Ok((Key::new(id, bridge_id), message_id_bytes))
}

/// Shared context for batch processing that remains constant across all batches.
struct BatchProcessContext<'a> {
    bridge_id: i32,
    chain_ids: &'a HashSet<i64>,
    process_unknown_chains: bool,
    blockchain_id_resolver: &'a BlockchainIdResolver,
    buffer: &'a Arc<MessageBuffer<Message>>,
}

/// Process a batch of logs for a single chain.
///
/// Logs are grouped by transaction hash so we can fetch the full receipt
/// (including non-Teleporter logs) and block timestamp once per tx.
async fn process_batch(
    batch: Vec<Log>,
    chain_id: i64,
    ctx: &BatchProcessContext<'_>,
    provider: &DynProvider<Ethereum>,
) -> Result<()> {
    let logs_by_transaction_hash: HashMap<B256, Vec<&Log>> = batch
        .iter()
        .filter_map(|log| log.transaction_hash.map(|hash| (hash, log)))
        .into_group_map();

    let transaction_hashes: HashSet<B256> = logs_by_transaction_hash.keys().copied().collect();

    let receipts_by_transaction_hash: HashMap<_, _> = stream::iter(transaction_hashes)
        .map(|hash| async move {
            let receipt = provider
                .get_transaction_receipt(hash)
                .await?
                .context("transaction receipt not found")?;

            let block_number = receipt
                .block_number
                .ok_or(anyhow::anyhow!("missing block number"))?
                .into();

            let block = provider
                .get_block_by_number(block_number)
                .await?
                .context("block not found")?;

            let logs = receipt.inner.logs().to_vec();
            Ok::<(B256, (Vec<Log>, Block)), anyhow::Error>((hash, (logs, block)))
        })
        .buffer_unordered(25)
        .try_collect()
        .await?;

    // Process each transaction's logs together
    for (hash, teleporter_logs) in logs_by_transaction_hash {
        let (receipt_logs, block) = receipts_by_transaction_hash
            .get(&hash)
            .context("missing receipt or block")?;

        let block_timestamp = chrono::DateTime::from_timestamp(block.header.timestamp as i64, 0)
            .map(|dt| dt.naive_utc())
            .context("invalid timestamp")?;

        // Process all Teleporter logs in this transaction.
        for log in teleporter_logs {
            let block_number = log.block_number.context("missing block number")? as i64;

            let log_ctx = LogHandleContext {
                chain_id,
                block_number,
                block_timestamp,
                log,
                bridge_id: ctx.bridge_id,
                chain_ids: ctx.chain_ids,
                process_unknown_chains: ctx.process_unknown_chains,
                blockchain_id_resolver: ctx.blockchain_id_resolver,
                buffer: ctx.buffer,
                receipt_logs,
            };

            handle_log(log_ctx).await?;
        }
    }

    Ok(())
}

/// Parse sender-side ICTT logs from a receipt.
///
/// These logs include `teleporterMessageID`, so we can associate them with the
/// in-flight message and capture a single sender-side transfer per message.
fn parse_sender_ictt_log(
    message_id: &B256,
    transfer: &Option<TokenTransfer>,
    log: &Log,
) -> Result<Option<TokenTransfer>> {
    let contract_address = log.address();
    let signature = log.topic0().context("missing topic0")?;
    let mismatch_error = "mismatched ICTT transfer types in single receipt";

    let transfer = match *signature {
        ITokenTransferrer::TokensSent::SIGNATURE_HASH => {
            let event = log
                .log_decode::<ITokenTransferrer::TokensSent>()?
                .inner
                .data;

            let withdrawn = match transfer {
                Some(TokenTransfer::Sent(_, withdrawn)) => Ok(withdrawn.clone()),
                None => Ok(None),
                _ => Err(anyhow!(mismatch_error)),
            }?;

            (&event.teleporterMessageID == message_id).then_some(TokenTransfer::Sent(
                Some(SentOrRouted::Sent(AnnotatedICTTSource {
                    event: event.clone(),
                    contract_address,
                })),
                withdrawn,
            ))
        }
        ITokenTransferrer::TokensAndCallSent::SIGNATURE_HASH => {
            let event = log
                .log_decode::<ITokenTransferrer::TokensAndCallSent>()?
                .inner
                .data;

            let withdrawn = match transfer {
                Some(TokenTransfer::SentAndCalled(_, withdrawn)) => Ok(withdrawn.clone()),
                None => Ok(None),
                _ => Err(anyhow!(mismatch_error)),
            }?;

            (&event.teleporterMessageID == message_id).then_some(TokenTransfer::SentAndCalled(
                Some(SentOrRoutedAndCalled::Sent(AnnotatedICTTSource {
                    event: event.clone(),
                    contract_address,
                })),
                withdrawn,
            ))
        }
        ITokenHome::TokensRouted::SIGNATURE_HASH => {
            let event = log.log_decode::<ITokenHome::TokensRouted>()?.inner.data;

            let withdrawn = match transfer {
                Some(TokenTransfer::Sent(_, withdrawn)) => Ok(withdrawn.clone()),
                None => Ok(None),
                _ => Err(anyhow!(mismatch_error)),
            }?;

            (&event.teleporterMessageID == message_id).then_some(TokenTransfer::Sent(
                Some(SentOrRouted::Routed(AnnotatedICTTSource {
                    event: event.clone(),
                    contract_address,
                })),
                withdrawn,
            ))
        }
        ITokenHome::TokensAndCallRouted::SIGNATURE_HASH => {
            let event = log
                .log_decode::<ITokenHome::TokensAndCallRouted>()?
                .inner
                .data;

            let withdrawn = match transfer {
                Some(TokenTransfer::SentAndCalled(_, withdrawn)) => Ok(withdrawn.clone()),
                None => Ok(None),
                _ => Err(anyhow!(mismatch_error)),
            }?;

            (&event.teleporterMessageID == message_id).then_some(TokenTransfer::SentAndCalled(
                Some(SentOrRoutedAndCalled::Routed(AnnotatedICTTSource {
                    event: event.clone(),
                    contract_address,
                })),
                withdrawn,
            ))
        }
        _ => None,
    };

    Ok(transfer)
}

/// Parse receiver-side ICTT outcome logs (which don't include
/// `teleporterMessageID`).
///
/// Callers must enforce the single-outcome invariant first.
fn parse_receiver_ictt_logs(
    transfer: &Option<TokenTransfer>,
    logs: &[Log],
) -> Result<Option<TokenTransfer>> {
    let counts = logs
        .iter()
        .filter_map(|log| {
            let sig = log.topic0()?;
            [
                &ITokenTransferrer::TokensWithdrawn::SIGNATURE_HASH,
                &ITokenTransferrer::CallSucceeded::SIGNATURE_HASH,
                &ITokenTransferrer::CallFailed::SIGNATURE_HASH,
            ]
            .contains(&sig)
            .then_some(*sig)
        })
        .counts();

    counts
        .iter()
        .find(|(_, count)| **count > 1)
        .map_or(Ok(()), |(outcome, _)| {
            Err(anyhow!(
                "receiver-side invariant violated: multiple {} logs in one receipt",
                hex::encode(outcome)
            ))
        })?;

    if counts.contains_key(&ITokenTransferrer::CallSucceeded::SIGNATURE_HASH)
        && counts.contains_key(&ITokenTransferrer::CallFailed::SIGNATURE_HASH)
    {
        Err(anyhow!(
            "receiver-side invariant violated: both CallSucceeded and CallFailed present in one receipt"
        ))?;
    }

    // TODO: add a regression test to check that fallback to original transfer
    // works as expected
    logs.iter()
        .find_map(|log| parse_receiver_ictt_log(transfer, log).transpose())
        .transpose()
        .map(|parsed| parsed.or_else(|| transfer.clone()))
}

fn parse_receiver_ictt_log(
    transfer: &Option<TokenTransfer>,
    log: &Log,
) -> Result<Option<TokenTransfer>> {
    let mismatch_error = "mismatched ICTT transfer types in single receipt";

    let transfer = match log.topic0() {
        Some(&ITokenTransferrer::TokensWithdrawn::SIGNATURE_HASH) => {
            let event = log.log_decode::<ITokenTransferrer::TokensWithdrawn>()?;

            let sent_or_routed = match transfer {
                Some(TokenTransfer::Sent(existing, _)) => Ok(existing.clone()),
                None => Ok(None),
                _ => Err(anyhow!(mismatch_error)),
            }?;

            Some(TokenTransfer::Sent(sent_or_routed, Some(event.inner.data)))
        }
        Some(&ITokenTransferrer::CallSucceeded::SIGNATURE_HASH) => {
            let event = log.log_decode::<ITokenTransferrer::CallSucceeded>()?;

            let sent_or_routed_and_called = match transfer {
                Some(TokenTransfer::SentAndCalled(existing, _)) => Ok(existing.clone()),
                None => Ok(None),
                _ => Err(anyhow!(mismatch_error)),
            }?;

            Some(TokenTransfer::SentAndCalled(
                sent_or_routed_and_called,
                Some(CallOutcome::Succeeded(event.inner.data)),
            ))
        }
        Some(&ITokenTransferrer::CallFailed::SIGNATURE_HASH) => {
            let event = log.log_decode::<ITokenTransferrer::CallFailed>()?;

            let sent_or_routed_and_called = match transfer {
                Some(TokenTransfer::SentAndCalled(existing, _)) => Ok(existing.clone()),
                None => Ok(None),
                _ => Err(anyhow!(mismatch_error)),
            }?;

            Some(TokenTransfer::SentAndCalled(
                sent_or_routed_and_called,
                Some(CallOutcome::Failed(event.inner.data)),
            ))
        }
        _ => None,
    };

    Ok(transfer)
}

/// Shared per-log handler context.
///
/// This bundles the (previously many) positional arguments passed through the
/// avalanche log handler call chain. It makes call sites clearer, reduces the
/// risk of argument-order bugs, and allows handler signatures to evolve without
/// rippling changes everywhere.
struct LogHandleContext<'a> {
    chain_id: i64,
    block_number: i64,
    block_timestamp: chrono::NaiveDateTime,
    bridge_id: i32,
    chain_ids: &'a HashSet<i64>,
    process_unknown_chains: bool,
    blockchain_id_resolver: &'a BlockchainIdResolver,
    buffer: &'a Arc<MessageBuffer<Message>>,

    /// Current log being handled.
    log: &'a Log,

    /// Full receipt logs for the transaction containing `log`.
    receipt_logs: &'a [Log],
}

/// Dispatch Teleporter ICM events to the appropriate handler.
///
/// Only Send/Receive/Execute events are handled here; other logs are ignored.
async fn handle_log(ctx: LogHandleContext<'_>) -> anyhow::Result<()> {
    if let Some(signature) = ctx.log.topic0() {
        match *signature {
            // ICM Source Event
            ITeleporterMessenger::SendCrossChainMessage::SIGNATURE_HASH => {
                handle_send_cross_chain_message(ctx).await
            }
            // ICM Destination Events - ReceiveCrossChainMessage may have execution in same tx
            ITeleporterMessenger::ReceiveCrossChainMessage::SIGNATURE_HASH => {
                handle_receive_cross_chain_message(ctx).await
            }
            // Execution events - handled for retry case (retryMessageExecution)
            // When execution comes via retry, it's in a different transaction than ReceiveCrossChainMessage
            ITeleporterMessenger::MessageExecuted::SIGNATURE_HASH => {
                handle_message_executed(ctx).await
            }
            ITeleporterMessenger::MessageExecutionFailed::SIGNATURE_HASH => {
                handle_message_execution_failed(ctx).await
            }
            _ => {
                tracing::trace!(?signature, "ignoring unknown event");
                Ok(())
            }
        }
    } else {
        tracing::warn!(
            block_number = ctx.block_number,
            chain_id = ctx.chain_id,
            "log missing topic0, cannot process"
        );
        Ok(())
    }
}

/// Handle SendCrossChainMessage - source-side event.
///
/// Also parses sender-side ICTT logs from the same receipt to capture
/// `TokensSent` / `TokensAndCallSent` (or routed variants).
async fn handle_send_cross_chain_message(ctx: LogHandleContext<'_>) -> Result<()> {
    let decoded = ctx
        .log
        .log_decode::<ITeleporterMessenger::SendCrossChainMessage>()?;
    let event = decoded.inner.data.clone();
    let transaction_hash = ctx
        .log
        .transaction_hash
        .context("missing transaction hash")?;
    let log_index = ctx.log.log_index.unwrap_or_default();
    let topic0 = ctx.log.topic0().context("missing topic0")?;

    let (key, message_id_bytes) = parse_message_key(&event.messageID, ctx.bridge_id)
        .context("failed to parse message key")?;

    let dst_chain_hex = event.destinationBlockchainID.as_slice();
    let dst_chain_id = ctx.blockchain_id_resolver.resolve(dst_chain_hex).await?;

    let destination_hex = hex::encode_prefixed(event.destinationBlockchainID.as_slice());

    let destination_chain_id = dst_chain_id;

    if !ctx.chain_ids.contains(&destination_chain_id) && !ctx.process_unknown_chains {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            destination_blockchain_id = %destination_hex,
            chain_id = ctx.chain_id,
            block_number = ctx.block_number,
            transaction_hash = %transaction_hash,
            log_index,
            signature = %topic0,
            destination_chain_id,
            "skipping SendCrossChainMessage to unknown chain"
        );
        return Ok(());
    }

    let chain_id = u64::try_from(ctx.chain_id).context("chain_id out of range")?;
    let block_number = u64::try_from(ctx.block_number).context("block_number out of range")?;

    ctx.buffer
        .alter(key, chain_id, block_number, |msg| {
        let transfers: Vec<TokenTransfer> = ctx.receipt_logs
            .iter()
            .filter_map(|log| parse_sender_ictt_log(&event.messageID, &msg.transfer, log).transpose())
            .collect::<Result<Vec<_>>>()?;

        let transfer = (transfers.len() <= 1).then_some(transfers.first()).context(
            "multiple sender-side ICTT transfers found for one teleporter message in a single receipt",
        )?.cloned();

        msg.send = Some(AnnotatedEvent{
            event,
            transaction_hash,
            block_number: ctx.block_number,
            block_timestamp: ctx.block_timestamp,
            source_chain_id: ctx.chain_id,
            destination_chain_id,
        });
        msg.transfer = transfer;
        Ok(())
    })
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id = ctx.chain_id,
        block_number = ctx.block_number,
        transaction_hash = %transaction_hash,
        log_index,
        signature = %topic0,
        destination_blockchain_id = %destination_hex,
        destination_chain_id,
        "processed SendCrossChainMessage"
    );

    Ok(())
}

fn parse_execution_outcome_log(
    logs: &[Log],
    source_chain_id: i64,
    destination_chain_id: i64,
    block_number: i64,
    block_timestamp: chrono::NaiveDateTime,
) -> Result<MessageExecutionOutcome> {
    logs.iter()
        .find_map(|log| {
            let transaction_hash = log.transaction_hash?;
            match *log.topic0()? {
                ITeleporterMessenger::MessageExecuted::SIGNATURE_HASH => log
                    .log_decode::<ITeleporterMessenger::MessageExecuted>()
                    .ok()
                    .map(|decoded| {
                        MessageExecutionOutcome::Succeeded(AnnotatedEvent {
                            event: decoded.inner.data.clone(),
                            transaction_hash,
                            block_number,
                            block_timestamp,
                            source_chain_id,
                            destination_chain_id,
                        })
                    }),
                ITeleporterMessenger::MessageExecutionFailed::SIGNATURE_HASH => log
                    .log_decode::<ITeleporterMessenger::MessageExecutionFailed>()
                    .ok()
                    .map(|decoded| {
                        MessageExecutionOutcome::Failed(
                            AnnotatedEvent {
                                event: decoded.inner.data.clone(),
                                transaction_hash,
                                block_number,
                                block_timestamp,
                                source_chain_id,
                                destination_chain_id,
                            }
                            .into(),
                        )
                    }),
                _ => None,
            }
        })
        .context("no execution outcome log found in receipt")
}

/// Handle ReceiveCrossChainMessage - destination-side event.
///
/// Records reception only. Execution outcome in the same transaction is
/// currently detected but not persisted (see refactor notes below).
async fn handle_receive_cross_chain_message(ctx: LogHandleContext<'_>) -> Result<()> {
    let decoded = ctx
        .log
        .log_decode::<ITeleporterMessenger::ReceiveCrossChainMessage>()?;
    let event = decoded.inner.data.clone();
    let transaction_hash = ctx
        .log
        .transaction_hash
        .context("missing transaction hash")?;
    let log_index = ctx.log.log_index.unwrap_or_default();
    let topic0 = ctx.log.topic0().copied().unwrap_or_default();

    let (key, message_id_bytes) = parse_message_key(&event.messageID, ctx.bridge_id)
        .context("failed to parse message key")?;

    let source_chain_id = event.sourceBlockchainID.as_slice();
    let source_chain_id = ctx.blockchain_id_resolver.resolve(source_chain_id).await?;

    let source_hex = hex::encode_prefixed(event.sourceBlockchainID.as_slice());

    if !ctx.chain_ids.contains(&source_chain_id) && !ctx.process_unknown_chains {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_hex,
            chain_id = ctx.chain_id,
            block_number = ctx.block_number,
            transaction_hash = %transaction_hash,
            log_index,
            signature = %topic0,
            source_chain_id,
            "skipping ReceiveCrossChainMessage from unknown chain"
        );
        return Ok(());
    }

    let destination_chain_id = ctx.chain_id;

    // Option A: ReceiveCrossChainMessage may have execution outcome in the same tx.
    // We keep `parse_execution_outcome_log` for now but it's intentionally unused until we
    // decide to fully wire the behaviour (see refactor notes).
    let _maybe_execution: Option<MessageExecutionOutcome> = ctx
        .receipt_logs
        .iter()
        .any(|l| {
            matches!(
                l.topic0(),
                Some(&ITeleporterMessenger::MessageExecuted::SIGNATURE_HASH)
                    | Some(&ITeleporterMessenger::MessageExecutionFailed::SIGNATURE_HASH)
            )
        })
        .then(|| {
            // Build lightweight annotated outcome without changing current persisted semantics.
            // Intentionally NOT persisted yet.
            parse_execution_outcome_log(
                ctx.receipt_logs,
                source_chain_id,
                destination_chain_id,
                ctx.block_number,
                ctx.block_timestamp,
            )
        })
        .transpose()
        .unwrap_or(None);

    let chain_id = u64::try_from(ctx.chain_id).context("chain_id out of range")?;
    let block_number = u64::try_from(ctx.block_number).context("block_number out of range")?;

    ctx.buffer
        .alter(key, chain_id, block_number, |msg| {
            msg.receive = Some(AnnotatedEvent {
                event,
                transaction_hash,
                block_number: ctx.block_number,
                block_timestamp: ctx.block_timestamp,
                source_chain_id,
                destination_chain_id,
            });
            Ok(())
        })
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id = ctx.chain_id,
        block_number = ctx.block_number,
        transaction_hash = %transaction_hash,
        log_index,
        signature = %topic0,
        source_blockchain_id = %source_hex,
        source_chain_id,
        "processed ReceiveCrossChainMessage"
    );

    Ok(())
}

/// Handle MessageExecuted - authoritative execution outcome.
///
/// This is the only place where receiver-side ICTT effects are parsed. It also
/// supports execution that happens in a separate transaction from receive.
async fn handle_message_executed(ctx: LogHandleContext<'_>) -> Result<()> {
    let decoded = ctx
        .log
        .log_decode::<ITeleporterMessenger::MessageExecuted>()?;
    let event = decoded.inner.data.clone();
    let transaction_hash = ctx
        .log
        .transaction_hash
        .context("missing transaction hash")?;
    let log_index = ctx.log.log_index.unwrap_or_default();
    let topic0 = ctx.log.topic0().copied().unwrap_or_default();

    let (key, message_id_bytes) = parse_message_key(&event.messageID, ctx.bridge_id)
        .context("failed to parse message key")?;

    let src_chain_id = event.sourceBlockchainID.as_slice();
    let src_chain_id = ctx.blockchain_id_resolver.resolve(src_chain_id).await?;

    let source_hex = hex::encode_prefixed(event.sourceBlockchainID.as_slice());

    let source_chain_id = src_chain_id;

    if !ctx.chain_ids.contains(&source_chain_id) && !ctx.process_unknown_chains {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_hex,
            chain_id = ctx.chain_id,
            block_number = ctx.block_number,
            transaction_hash = %transaction_hash,
            log_index,
            signature = %topic0,
            source_chain_id,
            "skipping MessageExecuted from unknown chain"
        );
        return Ok(());
    }

    let destination_chain_id = ctx.chain_id;

    let chain_id = u64::try_from(ctx.chain_id).context("chain_id out of range")?;
    let block_number = u64::try_from(ctx.block_number).context("block_number out of range")?;

    ctx.buffer
        .alter(key, chain_id, block_number, |msg| {
            // Receiver-side effects are parsed on MessageExecuted only.
            // Enforce a strict invariant to avoid ambiguous attribution.
            msg.execution = Some(MessageExecutionOutcome::Succeeded(AnnotatedEvent {
                event,
                transaction_hash,
                block_number: ctx.block_number,
                block_timestamp: ctx.block_timestamp,
                source_chain_id,
                destination_chain_id,
            }));
            msg.transfer = parse_receiver_ictt_logs(&msg.transfer, ctx.receipt_logs)?;
            Ok(())
        })
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id = ctx.chain_id,
        block_number = ctx.block_number,
        transaction_hash = %transaction_hash,
        log_index,
        signature = %topic0,
        source_blockchain_id = %source_hex,
        source_chain_id,
        "processed MessageExecuted"
    );

    Ok(())
}

/// Handle MessageExecutionFailed - execution failed.
///
/// The failure is recorded only if we have not already observed success for
/// the same message.
async fn handle_message_execution_failed(ctx: LogHandleContext<'_>) -> Result<()> {
    let decoded = ctx
        .log
        .log_decode::<ITeleporterMessenger::MessageExecutionFailed>()?;
    let event = decoded.inner.data.clone();
    let transaction_hash = ctx
        .log
        .transaction_hash
        .context("missing transaction hash")?;
    let log_index = ctx.log.log_index.unwrap_or_default();
    let topic0 = ctx.log.topic0().copied().unwrap_or_default();

    let (key, message_id_bytes) = parse_message_key(&event.messageID, ctx.bridge_id)
        .context("failed to parse message key")?;

    let src_chain_id = event.sourceBlockchainID.as_slice();
    let src_chain_id = ctx.blockchain_id_resolver.resolve(src_chain_id).await?;

    let source_hex = hex::encode_prefixed(event.sourceBlockchainID.as_slice());

    let source_chain_id = src_chain_id;

    if !ctx.chain_ids.contains(&source_chain_id) && !ctx.process_unknown_chains {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_hex,
            chain_id = ctx.chain_id,
            block_number = ctx.block_number,
            transaction_hash = %transaction_hash,
            log_index,
            signature = %topic0,
            source_chain_id,
            "skipping MessageExecutionFailed from unknown chain"
        );
        return Ok(());
    }

    let destination_chain_id = ctx.chain_id;

    let chain_id = u64::try_from(ctx.chain_id).context("chain_id out of range")?;
    let block_number = u64::try_from(ctx.block_number).context("block_number out of range")?;

    ctx.buffer
        .alter(key, chain_id, block_number, |msg| {
            // Only update if not already succeeded (don't overwrite success with failure)
            if !matches!(msg.execution, Some(MessageExecutionOutcome::Succeeded(_))) {
                msg.execution = Some(MessageExecutionOutcome::Failed(
                    AnnotatedEvent {
                        event,
                        transaction_hash,
                        block_number: ctx.block_number,
                        block_timestamp: ctx.block_timestamp,
                        source_chain_id,
                        destination_chain_id,
                    }
                    .into(),
                ));
            }

            Ok(())
        })
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id = ctx.chain_id,
        block_number = ctx.block_number,
        transaction_hash = %transaction_hash,
        log_index,
        signature = %topic0,
        source_blockchain_id = %source_hex,
        source_chain_id,
        "processed MessageExecutionFailed"
    );

    Ok(())
}
