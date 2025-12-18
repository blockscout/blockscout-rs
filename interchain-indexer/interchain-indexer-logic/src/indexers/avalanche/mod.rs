pub mod abi;
pub mod consolidation;
pub mod types;

use alloy::{
    network::Ethereum,
    primitives::{Address, B256},
    providers::{DynProvider, Provider as _},
    rpc::types::{Block, Filter, Log},
    sol_types::SolEvent,
};
use anyhow::{Context, Result, anyhow};
use futures::{StreamExt, TryStreamExt, stream};
use itertools::Itertools;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
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

/// Extract the indexer message key from a Teleporter `messageID`.
///
/// Today we use the first 8 bytes as big-endian i64.
fn parse_message_key(message_id: &B256, bridge_id: i32) -> Result<(Key, [u8; 8])> {
    let message_id_bytes: [u8; 8] = message_id.as_slice()[..8].try_into()?;
    let id = i64::from_be_bytes(message_id_bytes);
    Ok((Key::new(id, bridge_id), message_id_bytes))
}

/// Resolve an on-chain blockchain ID (bytes32 in Teleporter events) into our local chain id.
///
/// Returns `None` when the chain isn't tracked by this indexer.
fn resolve_tracked_chain_id(
    native_blockchain_id: &[u8],
    native_id_to_chain_id: &HashMap<String, i64>,
) -> Option<i64> {
    let native_hex = blockchain_id_hex(native_blockchain_id);
    native_id_to_chain_id.get(&native_hex).copied()
}

async fn process_batch(
    batch: Vec<Log>,
    chain_id: i64,
    bridge_id: i32,
    native_id_to_chain_id: &HashMap<String, i64>,
    buffer: &Arc<MessageBuffer<Message>>,
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

            handle_log(
                chain_id,
                block_number,
                log,
                bridge_id,
                native_id_to_chain_id,
                buffer,
                receipt_logs,
                block_timestamp,
            )
            .await?;
        }
    }

    Ok(())
}

fn parse_sender_ictt_log(
    message_id: &B256,
    transfer: &Option<TokenTransfer>,
    log: &Log,
) -> Result<Option<TokenTransfer>> {
    let contract_address = log.address();
    let signature = log.topic0().context("missing topic0")?;
    let mismatch_error = "mismatched ICTT transfer types in single receipt";

    let transfer = match signature {
        &ITokenTransferrer::TokensSent::SIGNATURE_HASH => {
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
        &ITokenTransferrer::TokensAndCallSent::SIGNATURE_HASH => {
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
        &ITokenHome::TokensRouted::SIGNATURE_HASH => {
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
        &ITokenHome::TokensAndCallRouted::SIGNATURE_HASH => {
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

/// Parse receiver-side ICTT outcome logs (which don't include teleporterMessageID).
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
            .then(|| *sig)
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

    (counts.contains_key(&ITokenTransferrer::CallSucceeded::SIGNATURE_HASH)
        && counts.contains_key(&ITokenTransferrer::CallFailed::SIGNATURE_HASH))
    .then_some(Err(anyhow!(
        "receiver-side invariant violated: both CallSucceeded and CallFailed present in one receipt"
    )))
    .unwrap_or(Ok(()))?;

    logs.iter()
        .find_map(|log| parse_receiver_ictt_log(transfer, log).transpose())
        .transpose()
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
            };

            Some(TokenTransfer::Sent(sent_or_routed?, Some(event.inner.data)))
        }
        Some(&ITokenTransferrer::CallSucceeded::SIGNATURE_HASH) => {
            let event = log.log_decode::<ITokenTransferrer::CallSucceeded>()?;

            let sent_or_routed_and_called = match transfer {
                Some(TokenTransfer::SentAndCalled(existing, _)) => Ok(existing.clone()),
                None => Ok(None),
                _ => Err(anyhow!(mismatch_error)),
            };

            Some(TokenTransfer::SentAndCalled(
                sent_or_routed_and_called?,
                Some(CallOutcome::Succeeded(event.inner.data)),
            ))
        }
        Some(&ITokenTransferrer::CallFailed::SIGNATURE_HASH) => {
            let event = log.log_decode::<ITokenTransferrer::CallFailed>()?;

            let sent_or_routed_and_called = match transfer {
                Some(TokenTransfer::SentAndCalled(existing, _)) => Ok(existing.clone()),
                None => Ok(None),
                _ => Err(anyhow!(mismatch_error)),
            };

            Some(TokenTransfer::SentAndCalled(
                sent_or_routed_and_called?,
                Some(CallOutcome::Failed(event.inner.data)),
            ))
        }
        _ => None,
    };

    Ok(transfer)
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
    receipt_logs: &[Log],
    block_timestamp: chrono::NaiveDateTime,
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
                receipt_logs,
                block_timestamp,
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
                receipt_logs,
                block_timestamp,
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
                receipt_logs,
                block_timestamp,
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
                block_timestamp,
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
    receipt_logs: &[Log],
    block_timestamp: chrono::NaiveDateTime,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::SendCrossChainMessage>()?;
    let event = decoded.inner.data.clone();
    let transaction_hash = log.transaction_hash.context("missing transaction hash")?;

    let (key, message_id_bytes) =
        parse_message_key(&event.messageID, bridge_id).context("failed to parse message key")?;
    let dst_chain_id = resolve_tracked_chain_id(
        event.destinationBlockchainID.as_slice(),
        native_id_to_chain_id,
    );
    let destination_hex = blockchain_id_hex(event.destinationBlockchainID.as_slice());

    // Skip messages to untracked chains
    if dst_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            destination_blockchain_id = %destination_hex,
            "Skipping SendCrossChainMessage to untracked chain"
        );
        return Ok(());
    }

    buffer
        .alter(key, chain_id, block_number, |msg| {
        let transfers: Vec<TokenTransfer> = receipt_logs
            .iter()
            .filter_map(|log| parse_sender_ictt_log(&event.messageID, &msg.transfer, log).transpose())
            .collect::<Result<Vec<_>>>()?;

        let transfer = (transfers.len() <= 1).then_some(transfers.first()).context(
            "multiple sender-side ICTT transfers found for one teleporter message in a single receipt",
        )?.cloned();

        msg.send = Some(AnnotatedEvent {
            event,
            transaction_hash,
            block_number,
            block_timestamp,
            chain_id,
        });
        msg.transfer = transfer;
        Ok(())
    })
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        "Processed SendCrossChainMessage"
    );

    Ok(())
}

fn parse_execution_outcome_log(
    logs: &[Log],
    chain_id: i64,
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
                            chain_id,
                        })
                    }),
                ITeleporterMessenger::MessageExecutionFailed::SIGNATURE_HASH => log
                    .log_decode::<ITeleporterMessenger::MessageExecutionFailed>()
                    .ok()
                    .map(|decoded| {
                        MessageExecutionOutcome::Failed(AnnotatedEvent {
                            event: decoded.inner.data.clone(),
                            transaction_hash,
                            block_number,
                            block_timestamp,
                            chain_id,
                        })
                    }),
                _ => None,
            }
        })
        .context("no execution outcome log found in receipt")
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
    receipt_logs: &[Log],
    block_timestamp: chrono::NaiveDateTime,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::ReceiveCrossChainMessage>()?;
    let event = decoded.inner.data.clone();
    let transaction_hash = log.transaction_hash.context("missing transaction hash")?;

    let (key, message_id_bytes) =
        parse_message_key(&event.messageID, bridge_id).context("failed to parse message key")?;
    let src_chain_id =
        resolve_tracked_chain_id(event.sourceBlockchainID.as_slice(), native_id_to_chain_id);
    let source_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());

    // Skip messages from untracked chains
    if src_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_hex,
            "Skipping ReceiveCrossChainMessage from untracked chain"
        );
        return Ok(());
    }

    // Option A: ReceiveCrossChainMessage may have execution outcome in the same tx.
    // We keep `parse_execution_outcome_log` for now but it's intentionally unused until we
    // decide to fully wire the behaviour (see refactor notes).
    let _maybe_execution: Option<MessageExecutionOutcome> = receipt_logs
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
            parse_execution_outcome_log(receipt_logs, chain_id, block_number, block_timestamp)
        })
        .transpose()
        .unwrap_or(None);

    buffer
        .alter(key, chain_id, block_number, |msg| {
            msg.receive = Some(AnnotatedEvent {
                event,
                transaction_hash,
                block_number,
                block_timestamp,
                chain_id,
            });
            Ok(())
        })
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        "processed ReceiveCrossChainMessage"
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
    receipt_logs: &[Log],
    block_timestamp: chrono::NaiveDateTime,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::MessageExecuted>()?;
    let event = decoded.inner.data.clone();
    let transaction_hash = log.transaction_hash.context("missing transaction hash")?;

    let (key, message_id_bytes) =
        parse_message_key(&event.messageID, bridge_id).context("failed to parse message key")?;
    let src_chain_id =
        resolve_tracked_chain_id(event.sourceBlockchainID.as_slice(), native_id_to_chain_id);
    let source_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());

    // Skip messages from untracked chains
    if src_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_hex,
            "Skipping MessageExecuted from untracked chain"
        );
        return Ok(());
    }

    buffer
        .alter(key, chain_id, block_number, |msg| {
            // Receiver-side effects are parsed on MessageExecuted only.
            // Enforce a strict invariant to avoid ambiguous attribution.
            msg.execution = Some(MessageExecutionOutcome::Succeeded(AnnotatedEvent {
                event,
                transaction_hash,
                block_number,
                block_timestamp,
                chain_id,
            }));
            msg.transfer = parse_receiver_ictt_logs(&msg.transfer, receipt_logs)?;
            Ok(())
        })
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        "processed MessageExecuted"
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
    block_timestamp: chrono::NaiveDateTime,
) -> Result<()> {
    let decoded = log.log_decode::<ITeleporterMessenger::MessageExecutionFailed>()?;
    let event = decoded.inner.data.clone();
    let transaction_hash = log.transaction_hash.context("missing transaction hash")?;

    let (key, message_id_bytes) =
        parse_message_key(&event.messageID, bridge_id).context("failed to parse message key")?;
    let src_chain_id =
        resolve_tracked_chain_id(event.sourceBlockchainID.as_slice(), native_id_to_chain_id);
    let source_hex = blockchain_id_hex(event.sourceBlockchainID.as_slice());

    // Skip messages from untracked chains
    if src_chain_id.is_none() {
        tracing::trace!(
            message_id = %hex::encode(message_id_bytes),
            source_blockchain_id = %source_hex,
            "Skipping MessageExecutionFailed from untracked chain"
        );
        return Ok(());
    }

    buffer
        .alter(key, chain_id, block_number, |msg| {
            // Only update if not already succeeded (don't overwrite success with failure)
            if !matches!(msg.execution, Some(MessageExecutionOutcome::Succeeded(_))) {
                msg.execution = Some(MessageExecutionOutcome::Failed(AnnotatedEvent {
                    event,
                    transaction_hash,
                    block_number,
                    block_timestamp,
                    chain_id,
                }));
            }

            Ok(())
        })
        .await?;

    tracing::debug!(
        message_id = %hex::encode(message_id_bytes),
        chain_id,
        block_number,
        "Processed MessageExecutionFailed"
    );

    Ok(())
}
