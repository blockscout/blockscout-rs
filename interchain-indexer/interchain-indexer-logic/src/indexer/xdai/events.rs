use std::{collections::HashMap, sync::Arc};

use alloy::{
    dyn_abi::{DynSolValue, EventExt},
    primitives::{Address, B256, U256, keccak256},
    rpc::types::{Block, Log},
    sol_types::SolEvent,
};
use anyhow::{Context, Result, bail, ensure};
use dashmap::DashMap;

use crate::{
    indexer::evm::fetch_receipts_for_transactions,
    message_buffer::{Key, MessageBuffer},
};

use super::{
    abi::{AbiRegistry, LogResolution},
    indexer::XDaiChainConfig,
    types::{
        AnnotatedEvent, CollectedSignaturesEvent, Completion, CompletionEvent, Direction,
        LegacySourceEvent, Message, MessageIdentity, ReconstructedSource,
        UserRequestForAffirmationEvent, UserRequestForSignatureEvent, ValidatorConfirmation,
        compute_message_hash, key_from_native_id,
    },
    version::{XDaiSide, grammar_for, legacy_ethereum_asset},
};

alloy::sol! {
    event LegacyUserRequestForAffirmation(address recipient, uint256 value);
    event LegacyUserRequestForSignature(address recipient, uint256 value);
}

pub(super) struct EventContext<'a> {
    pub(super) bridge_id: i32,
    pub(super) chain_id: i64,
    /// The block every log in this context came from. Contract versions are
    /// resolved by `(address, block)`, so an upgraded proxy would otherwise
    /// decode against the wrong ABI.
    pub(super) block_number: u64,
    pub(super) abi_registry: &'a AbiRegistry,
    pub(super) buffer: &'a Arc<MessageBuffer<Message>>,
    /// Gno→Eth only: the Foreign proxy's own address, the `foreignBridgeAddr`
    /// component of the `messageHash` preimage.
    pub(super) foreign_bridge_address: Address,
    pub(super) message_hash_lookup: &'a Arc<DashMap<B256, Key>>,
    pub(super) pending_message_hash_events: &'a Arc<DashMap<B256, PendingMessageHashEvents>>,
    pub(super) counterpart_chain: Option<&'a XDaiChainConfig>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PendingMessageHashEvents {
    validator_confirmations: HashMap<Address, PendingValidatorConfirmation>,
    signatures_collected: Option<PendingCollectedSignatures>,
}

#[derive(Clone, Debug)]
struct PendingValidatorConfirmation {
    chain_id: u64,
    confirmation: ValidatorConfirmation,
}

#[derive(Clone, Debug)]
struct PendingCollectedSignatures {
    chain_id: u64,
    event: AnnotatedEvent<CollectedSignaturesEvent>,
}

/// Mirrors `amb/events.rs::dispatch_transaction`'s aggregation exactly: every
/// log in the transaction is still dispatched even after a handler failure,
/// and the aggregate failure (if any) is reported to the caller only once
/// every log has been tried. Swallowing a handler failure here would let the
/// retry path's `ledger.resolve` delete a hole whose data was never restored.
pub(super) async fn dispatch_transaction(
    ctx: &EventContext<'_>,
    receipt_logs: &[Log],
    block: &Block,
    transaction_from: Address,
) -> Result<()> {
    let block_timestamp = chrono::DateTime::from_timestamp(block.header.timestamp as i64, 0)
        .map(|dt| dt.naive_utc())
        .context("invalid block timestamp")?;

    let mut last_err: Option<anyhow::Error> = None;
    let mut failed_events = 0usize;

    for log in receipt_logs {
        let Some(topic) = log.topic0() else {
            continue;
        };
        let (event, kind) =
            match ctx
                .abi_registry
                .resolve_log(ctx.chain_id, log.address(), topic, ctx.block_number)
            {
                LogResolution::Matched(event, kind) => (event, kind),
                LogResolution::NotConfigured => continue,
                LogResolution::WrongVersion => {
                    // Protocol-labelled shared counter; see its doc in
                    // `indexer/metrics.rs`. xDai is new, so there is no
                    // legacy per-protocol counter to keep emitting alongside.
                    crate::indexer::metrics::LOGS_DROPPED_WRONG_VERSION_TOTAL
                        .with_label_values(&[
                            "xdai",
                            &ctx.bridge_id.to_string(),
                            &ctx.chain_id.to_string(),
                        ])
                        .inc();
                    tracing::warn!(
                        bridge_id = ctx.bridge_id,
                        chain_id = ctx.chain_id,
                        block_number = ctx.block_number,
                        tx_hash = ?log.transaction_hash,
                        log_index = ?log.log_index,
                        address = %log.address(),
                        "dropped an xDai log whose topic belongs to a different configured \
                         version of this contract; check started_at_block for that address"
                    );
                    continue;
                }
            };

        let result = match event.name.as_str() {
            "UserRequestForAffirmation" => {
                handle_user_request_for_affirmation(
                    ctx,
                    event,
                    kind.version,
                    log,
                    block_timestamp,
                    transaction_from,
                )
                .await
            }
            "SignedForAffirmation" => {
                handle_signed_for_affirmation(ctx, event, log, block_timestamp).await
            }
            "AffirmationCompleted" => {
                handle_affirmation_completed(ctx, event, log, block_timestamp).await
            }
            "UserRequestForSignature" => {
                handle_user_request_for_signature(
                    ctx,
                    event,
                    log,
                    block_timestamp,
                    transaction_from,
                )
                .await
            }
            "SignedForUserRequest" => {
                handle_signed_for_user_request(ctx, event, log, block_timestamp).await
            }
            "CollectedSignatures" => {
                handle_collected_signatures(ctx, event, log, block_timestamp).await
            }
            "RelayedMessage" => handle_relayed_message(ctx, event, log, block_timestamp).await,
            _ => Ok(()),
        };

        if let Err(err) = result {
            tracing::warn!(
                bridge_id = ctx.bridge_id,
                chain_id = ctx.chain_id,
                tx_hash = ?log.transaction_hash,
                log_index = ?log.log_index,
                event_name = event.name,
                err = ?err,
                "failed to process xDai event"
            );
            failed_events += 1;
            last_err = Some(err);
        }
    }

    if let Some(err) = last_err {
        return Err(err.context(format!(
            "{failed_events} xDai event handler(s) failed to process"
        )));
    }

    Ok(())
}

async fn handle_user_request_for_affirmation(
    ctx: &EventContext<'_>,
    event: &alloy::json_abi::Event,
    version: i16,
    log: &Log,
    block_timestamp: chrono::NaiveDateTime,
    transaction_from: Address,
) -> Result<()> {
    let decoded = event.decode_log(log.data())?;
    let recipient = expect_address(decoded.body.first(), "recipient")?;
    let value = expect_uint(decoded.body.get(1), "value")?;
    let nonce = expect_nonce(decoded.body.get(2), "nonce")?;
    let identity = MessageIdentity::source(nonce)?;

    // Never from a log (no token field exists) and never from a `latest`
    // RPC call (would relabel history): the version-block-keyed grammar
    // table is the only source that stays correct across the DAI->USDS
    // flip at Foreign v10.
    let source_asset = grammar_for(XDaiSide::Foreign, version)?
        .source_asset
        .context("xDai Foreign grammar has no source_asset")?;

    let native_id = identity.native_id(Direction::EthToGno)?;
    let key = key_from_native_id(&native_id, ctx.bridge_id)?;
    let block_number = log.block_number.context("missing block number")?;

    let annotated = AnnotatedEvent {
        event: UserRequestForAffirmationEvent {
            recipient,
            value,
            nonce,
            source_asset,
        },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
    };

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            ensure_identity_and_direction(message, identity, Direction::EthToGno)?;
            message.direction = Some(Direction::EthToGno);
            message.identity = Some(identity);
            message.source_request = Some(annotated);
            message.sender_address = Some(transaction_from);
            Ok(())
        })
        .await
}

async fn handle_signed_for_affirmation(
    ctx: &EventContext<'_>,
    event: &alloy::json_abi::Event,
    log: &Log,
    block_timestamp: chrono::NaiveDateTime,
) -> Result<()> {
    let decoded = event.decode_log(log.data())?;
    let signer = expect_address(decoded.indexed.first(), "signer")?;
    let nonce = expect_nonce(decoded.body.first(), "nonce")?;
    let identity = MessageIdentity::destination(nonce);

    let native_id = identity.native_id(Direction::EthToGno)?;
    let key = key_from_native_id(&native_id, ctx.bridge_id)?;
    let block_number = log.block_number.context("missing block number")?;

    let confirmation = ValidatorConfirmation {
        validator_address: signer,
        tx_hash: log.transaction_hash.context("missing tx hash")?,
        block_number,
        block_timestamp,
    };

    if let MessageIdentity::SourceTransactionHash(source_hash) = identity {
        tracing::warn!(
            bridge_id = ctx.bridge_id,
            chain_id = ctx.chain_id,
            block_number,
            tx_hash = ?log.transaction_hash,
            log_index = ?log.log_index,
            validator_address = %signer,
            source_tx_hash = %source_hash,
            "observed legacy xDai confirmation keyed by source transaction hash"
        );
    }

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            ensure_identity_and_direction(message, identity, Direction::EthToGno)?;
            message.identity = Some(identity);
            message.direction = Some(Direction::EthToGno);
            message.validator_confirmations.insert(signer, confirmation);
            Ok(())
        })
        .await
}

async fn handle_affirmation_completed(
    ctx: &EventContext<'_>,
    event: &alloy::json_abi::Event,
    log: &Log,
    block_timestamp: chrono::NaiveDateTime,
) -> Result<()> {
    let decoded = event.decode_log(log.data())?;
    let recipient = expect_address(decoded.body.first(), "recipient")?;
    let value = expect_uint(decoded.body.get(1), "value")?;
    let value_or_hash = expect_nonce(decoded.body.get(2), "nonce")?;
    let identity = MessageIdentity::destination(value_or_hash);

    let native_id = identity.native_id(Direction::EthToGno)?;
    let key = key_from_native_id(&native_id, ctx.bridge_id)?;
    let block_number = log.block_number.context("missing block number")?;

    let annotated = AnnotatedEvent {
        event: CompletionEvent { recipient, value },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
    };

    let completion = Completion::Affirmation(annotated);
    let reconstructed_source = match identity {
        MessageIdentity::Nonce(_) => None,
        MessageIdentity::SourceTransactionHash(source_hash) => {
            Some(reconstruct_source(ctx, Direction::EthToGno, source_hash, recipient).await?)
        }
    };

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            ensure_completion_compatible(
                message,
                identity,
                Direction::EthToGno,
                &completion,
                reconstructed_source.as_ref(),
            )?;
            message.identity = Some(identity);
            message.direction = Some(Direction::EthToGno);
            message.destination_execution = Some(completion);
            message.reconstructed_source = reconstructed_source;
            Ok(())
        })
        .await
}

async fn handle_user_request_for_signature(
    ctx: &EventContext<'_>,
    event: &alloy::json_abi::Event,
    log: &Log,
    block_timestamp: chrono::NaiveDateTime,
    transaction_from: Address,
) -> Result<()> {
    let decoded = event.decode_log(log.data())?;
    let recipient = expect_address(decoded.body.first(), "recipient")?;
    let value = expect_uint(decoded.body.get(1), "value")?;
    let nonce = expect_nonce(decoded.body.get(2), "nonce")?;
    let identity = MessageIdentity::source(nonce)?;
    let token = match decoded.body.get(3) {
        Some(DynSolValue::Address(token)) => Some(*token),
        None => None,
        other => bail!("expected optional address token, got {other:?}"),
    };

    let native_id = identity.native_id(Direction::GnoToEth)?;
    let key = key_from_native_id(&native_id, ctx.bridge_id)?;
    let block_number = log.block_number.context("missing block number")?;

    let annotated = AnnotatedEvent {
        event: UserRequestForSignatureEvent {
            recipient,
            value,
            nonce,
            token,
        },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
    };

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            ensure_identity_and_direction(message, identity, Direction::GnoToEth)?;
            message.identity = Some(identity);
            message.direction = Some(Direction::GnoToEth);
            message.signature_request = Some(annotated);
            message.sender_address = Some(transaction_from);
            Ok(())
        })
        .await?;

    // Computable at source time: every component is in the event or in
    // config, so the lookup is populated proactively rather than waiting for
    // `submitSignature`'s own blob.
    let message_hash =
        compute_message_hash(recipient, value, nonce, ctx.foreign_bridge_address, token);
    ctx.message_hash_lookup.insert(message_hash, key);
    drain_pending_message_hash_events(ctx, message_hash, key).await
}

async fn handle_signed_for_user_request(
    ctx: &EventContext<'_>,
    event: &alloy::json_abi::Event,
    log: &Log,
    block_timestamp: chrono::NaiveDateTime,
) -> Result<()> {
    let decoded = event.decode_log(log.data())?;
    let signer = expect_address(decoded.indexed.first(), "signer")?;
    let message_hash = expect_b256(decoded.body.first(), "messageHash")?;
    let block_number = log.block_number.context("missing block number")?;

    let confirmation = ValidatorConfirmation {
        validator_address: signer,
        tx_hash: log.transaction_hash.context("missing tx hash")?,
        block_number,
        block_timestamp,
    };

    match ctx.message_hash_lookup.get(&message_hash).map(|key| *key) {
        Some(key) => {
            apply_validator_confirmation(ctx, key, ctx.chain_id as u64, confirmation).await
        }
        None => {
            ctx.pending_message_hash_events
                .entry(message_hash)
                .or_default()
                .validator_confirmations
                .insert(
                    signer,
                    PendingValidatorConfirmation {
                        chain_id: ctx.chain_id as u64,
                        confirmation,
                    },
                );
            tracing::debug!(
                bridge_id = ctx.bridge_id,
                chain_id = ctx.chain_id,
                tx_hash = ?log.transaction_hash,
                log_index = ?log.log_index,
                message_hash = %message_hash,
                "queued xDai validator confirmation until source request is processed"
            );
            report_pending_queue_size(ctx);
            Ok(())
        }
    }
}

async fn handle_collected_signatures(
    ctx: &EventContext<'_>,
    event: &alloy::json_abi::Event,
    log: &Log,
    block_timestamp: chrono::NaiveDateTime,
) -> Result<()> {
    let decoded = event.decode_log(log.data())?;
    let authority = expect_address(decoded.body.first(), "authorityResponsibleForRelay")?;
    let message_hash = expect_b256(decoded.body.get(1), "messageHash")?;
    let count = expect_uint(decoded.body.get(2), "NumberOfCollectedSignatures")?;
    let block_number = log.block_number.context("missing block number")?;

    let annotated = AnnotatedEvent {
        event: CollectedSignaturesEvent {
            authority_responsible_for_relay: authority,
            message_hash,
            count,
        },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
    };

    match ctx.message_hash_lookup.get(&message_hash).map(|key| *key) {
        Some(key) => apply_collected_signatures(ctx, key, ctx.chain_id as u64, annotated).await,
        None => {
            ctx.pending_message_hash_events
                .entry(message_hash)
                .or_default()
                .signatures_collected = Some(PendingCollectedSignatures {
                chain_id: ctx.chain_id as u64,
                event: annotated,
            });
            tracing::debug!(
                bridge_id = ctx.bridge_id,
                chain_id = ctx.chain_id,
                tx_hash = ?log.transaction_hash,
                log_index = ?log.log_index,
                message_hash = %message_hash,
                "queued xDai collected-signatures event until source request is processed"
            );
            report_pending_queue_size(ctx);
            Ok(())
        }
    }
}

async fn handle_relayed_message(
    ctx: &EventContext<'_>,
    event: &alloy::json_abi::Event,
    log: &Log,
    block_timestamp: chrono::NaiveDateTime,
) -> Result<()> {
    let decoded = event.decode_log(log.data())?;
    let recipient = expect_address(decoded.body.first(), "recipient")?;
    let value = expect_uint(decoded.body.get(1), "value")?;
    let value_or_hash = expect_nonce(decoded.body.get(2), "transactionHash")?;
    let identity = MessageIdentity::destination(value_or_hash);

    let native_id = identity.native_id(Direction::GnoToEth)?;
    let key = key_from_native_id(&native_id, ctx.bridge_id)?;
    let block_number = log.block_number.context("missing block number")?;

    let annotated = AnnotatedEvent {
        event: CompletionEvent { recipient, value },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
    };

    let completion = Completion::Relayed(annotated);
    let reconstructed_source = match identity {
        MessageIdentity::Nonce(_) => None,
        MessageIdentity::SourceTransactionHash(source_hash) => {
            Some(reconstruct_source(ctx, Direction::GnoToEth, source_hash, recipient).await?)
        }
    };

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            ensure_completion_compatible(
                message,
                identity,
                Direction::GnoToEth,
                &completion,
                reconstructed_source.as_ref(),
            )?;
            message.identity = Some(identity);
            message.direction = Some(Direction::GnoToEth);
            message.destination_execution = Some(completion);
            message.reconstructed_source = reconstructed_source;
            Ok(())
        })
        .await
}

fn ensure_identity_and_direction(
    message: &Message,
    identity: MessageIdentity,
    direction: Direction,
) -> Result<()> {
    ensure!(
        message.identity.is_none_or(|existing| existing == identity),
        "conflicting xDai message identity"
    );
    ensure!(
        message
            .direction
            .is_none_or(|existing| existing == direction),
        "conflicting xDai message direction"
    );
    Ok(())
}

fn ensure_completion_compatible(
    message: &Message,
    identity: MessageIdentity,
    direction: Direction,
    completion: &Completion,
    reconstructed_source: Option<&ReconstructedSource>,
) -> Result<()> {
    ensure_identity_and_direction(message, identity, direction)?;
    ensure!(
        message
            .destination_execution
            .as_ref()
            .is_none_or(|existing| existing == completion),
        "conflicting xDai completion payload"
    );
    ensure!(
        message
            .reconstructed_source
            .as_ref()
            .is_none_or(|existing| Some(existing) == reconstructed_source),
        "conflicting xDai reconstructed source"
    );
    Ok(())
}

async fn reconstruct_source(
    ctx: &EventContext<'_>,
    direction: Direction,
    source_hash: B256,
    destination_recipient: Address,
) -> Result<ReconstructedSource> {
    let counterpart = ctx
        .counterpart_chain
        .context("missing counterpart xDai chain configuration for legacy source reconstruction")?;
    let source =
        fetch_reconstructed_source(counterpart, direction, source_hash, destination_recipient)
            .await?;
    if direction == Direction::GnoToEth && source.legacy_source_event.is_none() {
        tracing::warn!(
            bridge_id = ctx.bridge_id,
            chain_id = ctx.chain_id,
            block_number = ctx.block_number,
            source_chain_id = counterpart.chain_id,
            source_tx_hash = %source_hash,
            source_block_number = source.block_number,
            "xDai transfer source is incompletely indexed: source contract event version is \
             not recognized; using destination completion amount for source amount"
        );
    }
    Ok(source)
}

async fn fetch_reconstructed_source(
    counterpart: &XDaiChainConfig,
    direction: Direction,
    source_hash: B256,
    destination_recipient: Address,
) -> Result<ReconstructedSource> {
    let proxy_address = counterpart_proxy_address(counterpart)?;
    let mut receipts = fetch_receipts_for_transactions(&counterpart.provider, [source_hash], 1)
        .await
        .with_context(|| format!("failed to reconstruct xDai source {source_hash}"))?;
    let receipt = receipts
        .remove(&source_hash)
        .with_context(|| format!("missing fetched xDai source receipt {source_hash}"))?;
    let block_number = receipt.block.header.number;
    let block_timestamp =
        chrono::DateTime::from_timestamp(receipt.block.header.timestamp as i64, 0)
            .map(|timestamp| timestamp.naive_utc())
            .context("invalid reconstructed xDai source block timestamp")?;
    let legacy_source_event = decode_legacy_source_event(
        direction,
        proxy_address,
        &receipt.logs,
        destination_recipient,
    )?;

    Ok(ReconstructedSource {
        transaction_hash: source_hash,
        block_number,
        block_timestamp,
        sender_address: receipt.transaction_from,
        ethereum_asset: legacy_ethereum_asset(direction, block_number)?,
        legacy_source_event,
    })
}

fn counterpart_proxy_address(chain: &XDaiChainConfig) -> Result<Address> {
    let Some(first) = chain.contracts.first().map(|contract| contract.address) else {
        bail!(
            "counterpart xDai chain {} has no proxy contracts",
            chain.chain_id
        );
    };
    ensure!(
        chain
            .contracts
            .iter()
            .all(|contract| contract.address == first),
        "counterpart xDai chain {} has multiple proxy addresses",
        chain.chain_id
    );
    Ok(first)
}

fn decode_legacy_source_event(
    direction: Direction,
    proxy_address: Address,
    logs: &[Log],
    destination_recipient: Address,
) -> Result<Option<LegacySourceEvent>> {
    let legacy_topic = match direction {
        Direction::EthToGno => LegacyUserRequestForAffirmation::SIGNATURE_HASH,
        Direction::GnoToEth => LegacyUserRequestForSignature::SIGNATURE_HASH,
    };
    let matching = logs
        .iter()
        .filter(|log| log.address() == proxy_address && log.topic0() == Some(&legacy_topic))
        .collect::<Vec<_>>();
    ensure!(
        matching.len() <= 1,
        "multiple matching legacy xDai source events"
    );

    let decoded = matching
        .first()
        .map(|log| -> Result<LegacySourceEvent> {
            let event = match direction {
                Direction::EthToGno => {
                    let event = LegacyUserRequestForAffirmation::decode_log_validate(&log.inner)
                        .context("malformed legacy UserRequestForAffirmation")?;
                    LegacySourceEvent {
                        recipient: event.data.recipient,
                        value: event.data.value,
                    }
                }
                Direction::GnoToEth => {
                    let event = LegacyUserRequestForSignature::decode_log_validate(&log.inner)
                        .context("malformed legacy UserRequestForSignature")?;
                    LegacySourceEvent {
                        recipient: event.data.recipient,
                        value: event.data.value,
                    }
                }
            };
            ensure!(
                event.recipient == destination_recipient,
                "legacy xDai source recipient mismatch"
            );
            Ok(event)
        })
        .transpose()?;

    if decoded.is_none() {
        let modern_topics = match direction {
            Direction::EthToGno => [
                keccak256("UserRequestForAffirmation(address,uint256,bytes32)"),
                B256::ZERO,
            ],
            Direction::GnoToEth => [
                keccak256("UserRequestForSignature(address,uint256,bytes32)"),
                keccak256("UserRequestForSignature(address,uint256,bytes32,address)"),
            ],
        };
        ensure!(
            !logs.iter().any(|log| {
                log.address() == proxy_address
                    && log
                        .topic0()
                        .is_some_and(|topic| modern_topics.contains(topic))
            }),
            "source receipt contains unsupported modern xDai source-request grammar"
        );
    }

    Ok(decoded)
}

/// Publishes the correlation queue's occupancy. Called from every site that
/// inserts into or removes from `pending_message_hash_events`, because the
/// map is unbounded and its size is the only warning that the source
/// (Gnosis) stream is lagging far enough for the queue to matter.
fn report_pending_queue_size(ctx: &EventContext<'_>) {
    crate::indexer::xdai::metrics::XDAI_PENDING_CORRELATION_QUEUE
        .with_label_values(&[&ctx.bridge_id.to_string()])
        .set(ctx.pending_message_hash_events.len() as f64);
}

/// Identity of a queued collected-signatures event, mirroring
/// `amb/events.rs`'s reason for existing: enough to tell the one a drain
/// snapshotted from a replacement queued during that drain's awaits.
type CollectedSignaturesIdentity = (u64, B256, i64, U256);

fn collected_signatures_identity(
    pending: &PendingCollectedSignatures,
) -> CollectedSignaturesIdentity {
    (
        pending.chain_id,
        pending.event.transaction_hash,
        pending.event.block_number,
        pending.event.event.count,
    )
}

/// Removes exactly the events a drain applied, leaving anything queued
/// during that drain's `.await`s in place. Returns `true` when the entry is
/// empty afterwards and the caller should remove it from the map. See
/// `amb/events.rs::remove_drained_events` for the full rationale (identical
/// here): validator confirmations are removed by signer (re-queuing
/// overwrites, so a same-signer re-queue is not a distinct event), while
/// `signatures_collected` needs an identity comparison because a
/// *replacement* queued during the awaits is a distinct event.
fn remove_drained_events(
    entry: &mut PendingMessageHashEvents,
    drained_confirmations: &[Address],
    drained_signatures: Option<&CollectedSignaturesIdentity>,
) -> bool {
    for signer in drained_confirmations {
        entry.validator_confirmations.remove(signer);
    }

    if let Some(drained) = drained_signatures
        && entry
            .signatures_collected
            .as_ref()
            .is_some_and(|current| collected_signatures_identity(current) == *drained)
    {
        entry.signatures_collected = None;
    }

    entry.validator_confirmations.is_empty() && entry.signatures_collected.is_none()
}

/// Applies the queued events for `message_hash`, then compare-and-removes
/// exactly what was applied from the queue entry.
///
/// The entry is deliberately **not** removed up front: clone → apply →
/// conditionally remove. Removing first makes a mid-drain failure
/// unrecoverable -- the blocks that carried the queued events were processed
/// successfully and are therefore not in the failure ledger, so a replay of
/// *this* block finds nothing queued and `resolve` deletes a hole whose data
/// was never restored. See `amb/events.rs::drain_pending_message_hash_events`
/// for the full argument (ADR-005) and why this ordering must not be
/// inverted -- only the removal's conditionality differs there for AMB's
/// collision handling, which xDai has no equivalent of (the nonce is
/// contract-issued, so a duplicate key can only be an indexing bug).
async fn drain_pending_message_hash_events(
    ctx: &EventContext<'_>,
    message_hash: B256,
    key: Key,
) -> Result<()> {
    let Some(pending) = ctx
        .pending_message_hash_events
        .get(&message_hash)
        .map(|entry| entry.value().clone())
    else {
        return Ok(());
    };
    let confirmation_count = pending.validator_confirmations.len();
    let has_signatures_collected = pending.signatures_collected.is_some();

    let drained_confirmations: Vec<Address> =
        pending.validator_confirmations.keys().copied().collect();
    let drained_signatures = pending
        .signatures_collected
        .as_ref()
        .map(collected_signatures_identity);

    for pending_confirmation in pending.validator_confirmations.into_values() {
        apply_validator_confirmation(
            ctx,
            key,
            pending_confirmation.chain_id,
            pending_confirmation.confirmation,
        )
        .await?;
    }

    if let Some(signatures_collected) = pending.signatures_collected {
        apply_collected_signatures(
            ctx,
            key,
            signatures_collected.chain_id,
            signatures_collected.event,
        )
        .await?;
    }

    let entry_is_now_empty = ctx
        .pending_message_hash_events
        .get_mut(&message_hash)
        .map(|mut entry| {
            remove_drained_events(
                entry.value_mut(),
                &drained_confirmations,
                drained_signatures.as_ref(),
            )
        })
        .unwrap_or(false);

    if entry_is_now_empty {
        ctx.pending_message_hash_events
            .remove_if(&message_hash, |_, current| {
                current.validator_confirmations.is_empty() && current.signatures_collected.is_none()
            });
    }

    report_pending_queue_size(ctx);

    tracing::debug!(
        bridge_id = ctx.bridge_id,
        chain_id = ctx.chain_id,
        message_hash = %message_hash,
        confirmation_count,
        has_signatures_collected,
        "drained queued xDai message-hash events"
    );

    Ok(())
}

async fn apply_validator_confirmation(
    ctx: &EventContext<'_>,
    key: Key,
    chain_id: u64,
    confirmation: ValidatorConfirmation,
) -> Result<()> {
    let block_number = confirmation.block_number;
    ctx.buffer
        .alter(key, chain_id, block_number, |message| {
            message
                .validator_confirmations
                .insert(confirmation.validator_address, confirmation);
            Ok(())
        })
        .await
}

async fn apply_collected_signatures(
    ctx: &EventContext<'_>,
    key: Key,
    chain_id: u64,
    annotated: AnnotatedEvent<CollectedSignaturesEvent>,
) -> Result<()> {
    let block_number = u64::try_from(annotated.block_number)
        .context("collected-signatures block number out of range")?;
    ctx.buffer
        .alter(key, chain_id, block_number, |message| {
            message.signatures_collected = Some(annotated);
            Ok(())
        })
        .await
}

fn expect_address(value: Option<&DynSolValue>, name: &str) -> Result<Address> {
    match value {
        Some(DynSolValue::Address(value)) => Ok(*value),
        other => bail!("expected address {name}, got {other:?}"),
    }
}

fn expect_uint(value: Option<&DynSolValue>, name: &str) -> Result<U256> {
    match value {
        Some(DynSolValue::Uint(value, _)) => Ok(*value),
        other => bail!("expected uint {name}, got {other:?}"),
    }
}

fn expect_b256(value: Option<&DynSolValue>, name: &str) -> Result<B256> {
    match value {
        Some(DynSolValue::FixedBytes(value, 32)) => Ok(*value),
        other => bail!("expected bytes32 {name}, got {other:?}"),
    }
}

/// The bridge declares its per-contract nonce as `bytes32` (`bytes32(currentNonce)`
/// in Solidity — a reinterpretation of the same 32 bytes, not a hash), so it
/// decodes as a fixed-bytes value that is read back as a big-endian `U256`.
fn expect_nonce(value: Option<&DynSolValue>, name: &str) -> Result<U256> {
    match value {
        Some(DynSolValue::FixedBytes(value, 32)) => Ok(U256::from_be_slice(value.as_slice())),
        other => bail!("expected bytes32 {name}, got {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use alloy::{
        primitives::{Bytes, LogData},
        providers::{Provider, ProviderBuilder},
        rpc::types::TransactionReceipt,
        transports::mock::Asserter,
    };

    use super::*;

    fn rpc_log(address: Address, data: LogData) -> Log {
        Log {
            inner: alloy::primitives::Log { address, data },
            ..Default::default()
        }
    }

    #[test]
    fn legacy_decoder_decodes_both_directions_and_preserves_source_value() {
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);
        let affirmation = LegacyUserRequestForAffirmation {
            recipient,
            value: U256::from(1_001u64),
        };
        let signature = LegacyUserRequestForSignature {
            recipient,
            value: U256::from(2_002u64),
        };

        assert_eq!(
            decode_legacy_source_event(
                Direction::EthToGno,
                proxy,
                &[rpc_log(proxy, affirmation.encode_log_data())],
                recipient,
            )
            .unwrap(),
            Some(LegacySourceEvent {
                recipient,
                value: U256::from(1_001u64)
            })
        );
        assert_eq!(
            decode_legacy_source_event(
                Direction::GnoToEth,
                proxy,
                &[rpc_log(proxy, signature.encode_log_data())],
                recipient,
            )
            .unwrap(),
            Some(LegacySourceEvent {
                recipient,
                value: U256::from(2_002u64)
            })
        );
    }

    #[test]
    fn legacy_decoder_ignores_another_proxy_and_allows_missing_source_fallback() {
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);
        let log = rpc_log(
            Address::repeat_byte(3),
            LegacyUserRequestForAffirmation {
                recipient,
                value: U256::ONE,
            }
            .encode_log_data(),
        );
        assert_eq!(
            decode_legacy_source_event(
                Direction::EthToGno,
                proxy,
                std::slice::from_ref(&log),
                recipient,
            )
            .unwrap(),
            None
        );
        assert_eq!(
            decode_legacy_source_event(Direction::GnoToEth, proxy, &[log], recipient).unwrap(),
            None
        );
    }

    #[test]
    fn legacy_decoder_rejects_malformed_multiple_and_recipient_mismatch() {
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);
        let valid = rpc_log(
            proxy,
            LegacyUserRequestForSignature {
                recipient,
                value: U256::ONE,
            }
            .encode_log_data(),
        );
        let malformed = rpc_log(
            proxy,
            LogData::new_unchecked(
                vec![LegacyUserRequestForSignature::SIGNATURE_HASH],
                Bytes::from_static(&[0u8; 1]),
            ),
        );
        assert!(
            decode_legacy_source_event(Direction::GnoToEth, proxy, &[malformed], recipient,)
                .is_err()
        );
        assert!(
            decode_legacy_source_event(
                Direction::GnoToEth,
                proxy,
                &[valid.clone(), valid.clone()],
                recipient,
            )
            .is_err()
        );
        assert!(
            decode_legacy_source_event(
                Direction::GnoToEth,
                proxy,
                &[valid],
                Address::repeat_byte(4),
            )
            .is_err()
        );
    }

    #[test]
    fn modern_source_request_never_becomes_plain_fallback() {
        let proxy = Address::repeat_byte(1);
        let modern = rpc_log(
            proxy,
            LogData::new_unchecked(
                vec![keccak256(
                    "UserRequestForAffirmation(address,uint256,bytes32)",
                )],
                Bytes::from(vec![0u8; 96]),
            ),
        );
        assert!(
            decode_legacy_source_event(
                Direction::EthToGno,
                proxy,
                &[modern],
                Address::repeat_byte(2),
            )
            .is_err()
        );
    }

    #[rstest::rstest]
    #[case(true)]
    #[case(false)]
    #[tokio::test]
    async fn reconstructed_source_uses_counterpart_provider_receipt_and_block(
        #[case] recognized_source_event: bool,
    ) {
        let asserter = Asserter::new();
        let provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);
        let sender = Address::repeat_byte(3);
        let source_hash = B256::repeat_byte(0x35);
        let mut source_log = rpc_log(
            proxy,
            LegacyUserRequestForSignature {
                recipient,
                value: U256::from(49_240u64),
            }
            .encode_log_data(),
        );
        if !recognized_source_event {
            // A historical event topic outside the decoder's supported grammar.
            source_log.inner.data = LogData::new_unchecked(
                vec![keccak256("UserRequestForSignature(address,uint256)")],
                source_log.data().data.clone(),
            );
        }
        source_log.transaction_hash = Some(source_hash);
        source_log.block_number = Some(39_557_691);
        source_log.log_index = Some(79);
        let receipt: TransactionReceipt = serde_json::from_value(serde_json::json!({
            "type": "0x2",
            "status": "0x1",
            "cumulativeGasUsed": "0x1",
            "logsBloom": format!("0x{}", "00".repeat(256)),
            "logs": [source_log],
            "transactionHash": source_hash,
            "transactionIndex": "0x0",
            "blockHash": B256::repeat_byte(4),
            "blockNumber": "0x25b9a3b",
            "gasUsed": "0x1",
            "effectiveGasPrice": "0x1",
            "from": sender,
            "to": proxy,
            "contractAddress": null
        }))
        .unwrap();
        asserter.push_success(&Some(receipt));

        let mut block: Block = Block::default();
        block.header.inner.number = 39_557_691;
        block.header.inner.timestamp = 1_744_646_380;
        asserter.push_success(&Some(block));

        let counterpart = XDaiChainConfig {
            chain_id: 100,
            provider,
            start_block: 39_569_937,
            contracts: vec![super::super::indexer::XDaiContractConfig {
                address: proxy,
                version: 7,
                started_at_block: 39_569_937,
                abi: None,
            }],
        };
        let reconstructed =
            fetch_reconstructed_source(&counterpart, Direction::GnoToEth, source_hash, recipient)
                .await
                .unwrap();

        assert_eq!(reconstructed.transaction_hash, source_hash);
        assert_eq!(reconstructed.block_number, 39_557_691);
        assert_eq!(reconstructed.sender_address, sender);
        assert_eq!(reconstructed.ethereum_asset, super::super::version::DAI);
        assert_eq!(
            reconstructed.legacy_source_event,
            recognized_source_event.then_some(LegacySourceEvent {
                recipient,
                value: U256::from(49_240u64),
            })
        );
        assert!(asserter.read_q().is_empty());
    }
}
