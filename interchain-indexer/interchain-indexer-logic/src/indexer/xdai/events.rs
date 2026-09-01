use std::{collections::HashMap, sync::Arc};

use alloy::{
    dyn_abi::{DynSolValue, EventExt},
    primitives::{Address, B256, U256},
    rpc::types::{Block, Log},
};
use anyhow::{Context, Result, bail};
use dashmap::DashMap;

use crate::message_buffer::{Key, MessageBuffer};

use super::{
    abi::{AbiRegistry, LogResolution},
    types::{
        AnnotatedEvent, CollectedSignaturesEvent, Completion, CompletionEvent, Direction, Message,
        UserRequestForAffirmationEvent, UserRequestForSignatureEvent, ValidatorConfirmation,
        compute_message_hash, key_from_native_id, native_id_blob,
    },
};

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
        let (event, _kind) =
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
    log: &Log,
    block_timestamp: chrono::NaiveDateTime,
    transaction_from: Address,
) -> Result<()> {
    let decoded = event.decode_log(log.data())?;
    let recipient = expect_address(decoded.body.first(), "recipient")?;
    let value = expect_uint(decoded.body.get(1), "value")?;
    let nonce = expect_nonce(decoded.body.get(2), "nonce")?;

    let native_id = native_id_blob(Direction::EthToGno.initiator_chain_id(), nonce)?;
    let key = key_from_native_id(&native_id, ctx.bridge_id)?;
    let block_number = log.block_number.context("missing block number")?;

    let annotated = AnnotatedEvent {
        event: UserRequestForAffirmationEvent {
            recipient,
            value,
            nonce,
        },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
    };

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            message.direction = Some(Direction::EthToGno);
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

    let native_id = native_id_blob(Direction::EthToGno.initiator_chain_id(), nonce)?;
    let key = key_from_native_id(&native_id, ctx.bridge_id)?;
    let block_number = log.block_number.context("missing block number")?;

    let confirmation = ValidatorConfirmation {
        validator_address: signer,
        tx_hash: log.transaction_hash.context("missing tx hash")?,
        block_number,
        block_timestamp,
    };

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
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
    let nonce = expect_nonce(decoded.body.get(2), "nonce")?;

    let native_id = native_id_blob(Direction::EthToGno.initiator_chain_id(), nonce)?;
    let key = key_from_native_id(&native_id, ctx.bridge_id)?;
    let block_number = log.block_number.context("missing block number")?;

    let annotated = AnnotatedEvent {
        event: CompletionEvent { recipient, value },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
    };

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            message.destination_execution = Some(Completion::Affirmation(annotated));
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
    let token = match decoded.body.get(3) {
        Some(DynSolValue::Address(token)) => Some(*token),
        None => None,
        other => bail!("expected optional address token, got {other:?}"),
    };

    let native_id = native_id_blob(Direction::GnoToEth.initiator_chain_id(), nonce)?;
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
    // The ABI still names this parameter `transactionHash` and the
    // Solidity comment still calls it one, but since Foreign v9 / Home v6 it
    // carries the Home (Gnosis) nonce -- see the protocol primer.
    let nonce = expect_nonce(decoded.body.get(2), "transactionHash")?;

    let native_id = native_id_blob(Direction::GnoToEth.initiator_chain_id(), nonce)?;
    let key = key_from_native_id(&native_id, ctx.bridge_id)?;
    let block_number = log.block_number.context("missing block number")?;

    let annotated = AnnotatedEvent {
        event: CompletionEvent { recipient, value },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
    };

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            message.destination_execution = Some(Completion::Relayed(annotated));
            Ok(())
        })
        .await
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
