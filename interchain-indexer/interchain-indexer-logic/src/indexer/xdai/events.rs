use std::{collections::HashMap, sync::Arc};

use alloy::{
    dyn_abi::{DynSolValue, EventExt},
    primitives::{Address, B256, U256},
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
        LegacySourceEvent, Message, MessageIdentity, ObservedExecution, ReconstructedSource,
        UserRequestForAffirmationEvent, UserRequestForSignatureEvent, ValidatorConfirmation,
        compute_message_hash, key_from_native_id,
    },
    version::{XDaiSide, grammar_for, legacy_ethereum_asset},
};

/// Real on-chain source-request event signatures, split into one `sol!` block
/// per distinct signature.
///
/// Solidity itself gives the two-argument (`legacy`) and three/four-argument
/// (`modern_*`) forms the *same* event name -- `UserRequestForAffirmation` /
/// `UserRequestForSignature` -- across deployment history; only their argument
/// lists (and therefore their `topic0`) differ. `alloy::sol!` names its
/// generated Rust type after the event, so two same-named events cannot share
/// one macro invocation. Splitting them into their own modules keeps every
/// declared name the **real** Solidity name (no invented `Legacy` prefix,
/// which previously hashed to a topic0 no deployed contract ever emits — see
/// the gotcha on this in `.memory-bank/gotchas.md`) while still giving each
/// form a distinct Rust path.
mod source_events {
    /// The real two-argument forms, pre-nonce.
    pub(super) mod legacy {
        alloy::sol! {
            event UserRequestForAffirmation(address recipient, uint256 value);
            event UserRequestForSignature(address recipient, uint256 value);
        }
    }
    pub(super) mod modern_affirmation {
        alloy::sol! {
            event UserRequestForAffirmation(address recipient, uint256 value, bytes32 nonce);
        }
    }
    /// Home v6: no `token` argument.
    pub(super) mod modern_signature_v6 {
        alloy::sol! {
            event UserRequestForSignature(address recipient, uint256 value, bytes32 nonce);
        }
    }
    /// Home v7+: adds `token`.
    pub(super) mod modern_signature_v7 {
        alloy::sol! {
            event UserRequestForSignature(address recipient, uint256 value, bytes32 nonce, address token);
        }
    }
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
    // flip at Foreign v10. `ctx.chain_id` is the Foreign chain here -- the
    // event only resolves on the Foreign side -- and selects the deployment.
    let source_asset = grammar_for(ctx.chain_id, XDaiSide::Foreign, version)?
        .source_asset
        .context("xDai Foreign grammar has no source_asset")?;

    let chain_ids = ctx.abi_registry.chain_ids()?;
    let native_id = identity.native_id(Direction::EthToGno, chain_ids)?;
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
            message.chain_ids = Some(chain_ids);
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

    let chain_ids = ctx.abi_registry.chain_ids()?;
    let native_id = identity.native_id(Direction::EthToGno, chain_ids)?;
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
            message.chain_ids = Some(chain_ids);
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
    let observed_identity = MessageIdentity::destination(value_or_hash);
    let log_index = log.log_index.map(|index| index as i64);
    let block_number = log.block_number.context("missing block number")?;

    let annotated = AnnotatedEvent {
        event: CompletionEvent { recipient, value },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
    };
    let completion = Completion::Affirmation(annotated);

    // Reconstruction (an RPC round trip) only happens for a hash-keyed
    // observation, and only decides the *canonical* identity -- the raw
    // observed one is preserved separately for provenance/anomaly bookkeeping
    // below. See `resolve_canonical_completion_identity`.
    let canonical = resolve_canonical_completion_identity(
        ctx,
        Direction::EthToGno,
        observed_identity,
        recipient,
    )
    .await?;
    let chain_ids = ctx.abi_registry.chain_ids()?;
    let (canonical_identity, reconstructed_source, nonce_evidence) = match canonical {
        CanonicalCompletionIdentity::Nonce { nonce, evidence } => {
            (MessageIdentity::Nonce(nonce), None, evidence)
        }
        CanonicalCompletionIdentity::SourceTransactionHash(source) => {
            let identity = MessageIdentity::SourceTransactionHash(source.transaction_hash);
            (identity, Some(source), None)
        }
    };
    let native_id = canonical_identity.native_id(Direction::EthToGno, chain_ids)?;
    let key = key_from_native_id(&native_id, ctx.bridge_id)?;

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            ensure_completion_compatible(
                message,
                canonical_identity,
                observed_identity,
                log_index,
                Direction::EthToGno,
                completion,
                reconstructed_source,
            )?;
            message.identity = Some(canonical_identity);
            message.direction = Some(Direction::EthToGno);
            message.chain_ids = Some(chain_ids);
            if let Some(NonceEvidence { event, facts }) = nonce_evidence {
                // Resolved the same way the stream path resolves it
                // (`handle_user_request_for_affirmation`), through the same
                // `AbiRegistry` -- see `resolve_modern_source_asset`'s doc for
                // why this must not fall back to `legacy_ethereum_asset` as
                // the primary source.
                let source_asset = resolve_modern_source_asset(
                    ctx,
                    chain_ids.foreign,
                    ctx.foreign_bridge_address,
                    facts.block_number,
                )?;
                message.source_request = Some(AnnotatedEvent {
                    event: UserRequestForAffirmationEvent {
                        recipient: event.recipient,
                        value: event.value,
                        nonce: event.nonce,
                        source_asset,
                    },
                    transaction_hash: facts.transaction_hash,
                    block_number: facts.block_number as i64,
                    block_timestamp: facts.block_timestamp,
                });
                message.sender_address = Some(facts.sender_address);
            }
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

    let chain_ids = ctx.abi_registry.chain_ids()?;
    let native_id = identity.native_id(Direction::GnoToEth, chain_ids)?;
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
            message.chain_ids = Some(chain_ids);
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
    let observed_identity = MessageIdentity::destination(value_or_hash);
    let log_index = log.log_index.map(|index| index as i64);
    let block_number = log.block_number.context("missing block number")?;

    let annotated = AnnotatedEvent {
        event: CompletionEvent { recipient, value },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
    };
    let completion = Completion::Relayed(annotated);

    let canonical = resolve_canonical_completion_identity(
        ctx,
        Direction::GnoToEth,
        observed_identity,
        recipient,
    )
    .await?;
    let chain_ids = ctx.abi_registry.chain_ids()?;
    let (canonical_identity, reconstructed_source, nonce_evidence) = match canonical {
        CanonicalCompletionIdentity::Nonce { nonce, evidence } => {
            (MessageIdentity::Nonce(nonce), None, evidence)
        }
        CanonicalCompletionIdentity::SourceTransactionHash(source) => {
            let identity = MessageIdentity::SourceTransactionHash(source.transaction_hash);
            (identity, Some(source), None)
        }
    };
    let native_id = canonical_identity.native_id(Direction::GnoToEth, chain_ids)?;
    let key = key_from_native_id(&native_id, ctx.bridge_id)?;

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            ensure_completion_compatible(
                message,
                canonical_identity,
                observed_identity,
                log_index,
                Direction::GnoToEth,
                completion,
                reconstructed_source,
            )?;
            message.identity = Some(canonical_identity);
            message.direction = Some(Direction::GnoToEth);
            message.chain_ids = Some(chain_ids);
            if let Some(NonceEvidence { event, facts }) = nonce_evidence {
                // Home v6 has no `token` field; `UserRequestForSignatureEvent`
                // already models that as `Option<Address>`, so `event.token`
                // (`None` for the three-argument grammar) carries straight
                // through -- no `source_asset` question on this side (only
                // Foreign grammars carry one).
                message.signature_request = Some(AnnotatedEvent {
                    event: UserRequestForSignatureEvent {
                        recipient: event.recipient,
                        value: event.value,
                        nonce: event.nonce,
                        token: event.token,
                    },
                    transaction_hash: facts.transaction_hash,
                    block_number: facts.block_number as i64,
                    block_timestamp: facts.block_timestamp,
                });
                message.sender_address = Some(facts.sender_address);
            }
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
    message: &mut Message,
    canonical_identity: MessageIdentity,
    observed_identity: MessageIdentity,
    log_index: Option<i64>,
    direction: Direction,
    completion: Completion,
    reconstructed_source: Option<ReconstructedSource>,
) -> Result<()> {
    ensure_identity_and_direction(message, canonical_identity, direction)?;

    let incoming_tx_hash = completion.event().transaction_hash;
    let existing_tx_hash = message
        .destination_execution
        .as_ref()
        .map(|existing| existing.event().transaction_hash);

    match existing_tx_hash {
        // First execution seen under this canonical key: becomes canonical.
        None => {
            message.destination_observed_identity = Some(observed_identity);
            message.destination_log_index = log_index;
            message.destination_execution = Some(completion);
        }
        // Idempotent replay of the already-canonical execution: identity is
        // the destination transaction (log_index is provenance only), so
        // nothing changes and this is not an error.
        Some(existing_tx_hash) if existing_tx_hash == incoming_tx_hash => {}
        // A genuinely different destination transaction under the same
        // canonical key: a multiple-execution candidate, deduped by
        // transaction hash and kept in first-appearance order. No error --
        // that decision is made later, against the database, in
        // `message_buffer::persistence::reconcile_destination_executions`.
        Some(_) => {
            let already_recorded = message
                .additional_executions
                .iter()
                .any(|existing| existing.completion.event().transaction_hash == incoming_tx_hash);
            if !already_recorded {
                message.additional_executions.push(ObservedExecution {
                    observed_identity,
                    log_index,
                    completion,
                });
            }
        }
    }

    // Fill only if empty: a nonce-keyed observation never carries receipt
    // evidence (`reconstructed_source = None`), while a hash-keyed one whose
    // receipt did not yield a modern source event does. Either can arrive
    // first under the same canonical key, so this must not compare against
    // whatever is already stored -- only fill the gap.
    if message.reconstructed_source.is_none() {
        message.reconstructed_source = reconstructed_source;
    }

    Ok(())
}

/// What the source transaction's receipt says about its own source-request
/// event, for the hash-keyed reconstruction path. Ordered so the caller can
/// tell "no event recognized" apart from "recognized, but the real two-argument
/// legacy grammar" -- the two-state `Option` this replaces conflated them.
#[derive(Debug)]
enum SourceEvidence {
    /// Exactly one modern (nonce-carrying) source event, recipient matched.
    Modern(ModernSourceEvent),
    /// The real two-argument legacy event, recipient matched.
    Legacy(LegacySourceEvent),
    /// A supported plain-transfer deposit, or an unrecognized historical
    /// grammar -- indistinguishable from here, both fall back to the
    /// destination completion's own amount.
    Missing,
}

/// A modern (post-nonce) source-request event decoded from a source
/// transaction's receipt.
#[derive(Debug)]
struct ModernSourceEvent {
    recipient: Address,
    value: U256,
    /// Validated `<= u64::MAX` by [`MessageIdentity::source`] before this is
    /// constructed.
    nonce: U256,
    /// Only the four-argument Home v7 `UserRequestForSignature`; `None` for
    /// the three-argument Home v6 form and for `UserRequestForAffirmation`
    /// (which never carries a token).
    token: Option<Address>,
}

/// Canonical identity resolved from a source transaction's receipt, plus the
/// facts about that receipt an identity-`Nonce` result still needs recorded on
/// the buffered message.
enum ReconstructedSourceIdentity {
    /// The receipt contains exactly one modern source event whose recipient
    /// matched: canonical identity becomes this nonce, and the raw hash that
    /// led here stays only as observation provenance.
    Nonce {
        nonce: U256,
        event: ModernSourceEvent,
        facts: SourceFacts,
    },
    /// A legacy two-argument event, or a supported plain transfer / an
    /// unrecognized historical grammar: canonical identity stays the raw
    /// source transaction hash, exactly as before this task.
    SourceTransactionHash(ReconstructedSource),
}

/// Facts about a fetched source-transaction receipt, independent of which
/// [`SourceEvidence`] variant it decoded to.
struct SourceFacts {
    transaction_hash: B256,
    block_number: u64,
    block_timestamp: chrono::NaiveDateTime,
    sender_address: Address,
    /// Resolved via `legacy_ethereum_asset`. For a [`ReconstructedSourceIdentity::Nonce`]
    /// result this is a fallback value only -- see
    /// `resolve_modern_source_asset`'s doc for the primary resolution path
    /// and why `legacy_ethereum_asset` is not it for a modern event.
    ethereum_asset: Address,
}

async fn reconstruct_source(
    ctx: &EventContext<'_>,
    direction: Direction,
    source_hash: B256,
    destination_recipient: Address,
) -> Result<ReconstructedSourceIdentity> {
    let counterpart = ctx
        .counterpart_chain
        .context("missing counterpart xDai chain configuration for legacy source reconstruction")?;
    // The asset table is keyed on the *Foreign* chain in both directions, so
    // it cannot come from `counterpart` (which is the source chain, Home for a
    // Gno->Eth message).
    let foreign_chain_id = ctx.abi_registry.chain_ids()?.foreign;
    let identity = fetch_reconstructed_source(
        counterpart,
        foreign_chain_id,
        direction,
        source_hash,
        destination_recipient,
    )
    .await?;
    if let ReconstructedSourceIdentity::SourceTransactionHash(source) = &identity
        && direction == Direction::GnoToEth
        && source.legacy_source_event.is_none()
    {
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
    Ok(identity)
}

async fn fetch_reconstructed_source(
    counterpart: &XDaiChainConfig,
    foreign_chain_id: i64,
    direction: Direction,
    source_hash: B256,
    destination_recipient: Address,
) -> Result<ReconstructedSourceIdentity> {
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
    let evidence = decode_source_evidence(
        direction,
        proxy_address,
        &receipt.logs,
        destination_recipient,
    )?;

    let facts = SourceFacts {
        transaction_hash: source_hash,
        block_number,
        block_timestamp,
        sender_address: receipt.transaction_from,
        ethereum_asset: legacy_ethereum_asset(foreign_chain_id, direction, block_number)?,
    };

    Ok(match evidence {
        SourceEvidence::Modern(event) => ReconstructedSourceIdentity::Nonce {
            nonce: event.nonce,
            event,
            facts,
        },
        SourceEvidence::Legacy(legacy) => {
            ReconstructedSourceIdentity::SourceTransactionHash(ReconstructedSource {
                transaction_hash: facts.transaction_hash,
                block_number: facts.block_number,
                block_timestamp: facts.block_timestamp,
                sender_address: facts.sender_address,
                ethereum_asset: facts.ethereum_asset,
                legacy_source_event: Some(legacy),
            })
        }
        SourceEvidence::Missing => {
            ReconstructedSourceIdentity::SourceTransactionHash(ReconstructedSource {
                transaction_hash: facts.transaction_hash,
                block_number: facts.block_number,
                block_timestamp: facts.block_timestamp,
                sender_address: facts.sender_address,
                ethereum_asset: facts.ethereum_asset,
                legacy_source_event: None,
            })
        }
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

/// Decodes what a source transaction's receipt says about its own
/// source-request event. Modern (nonce-carrying) grammar is checked **first**
/// -- checking legacy first could let a modern receipt fall through to the
/// `Missing` fallback and mint a duplicate logical transfer under a raw-hash
/// identity. `value` is neither a gate nor a tie-breaker anywhere in this
/// function; the only correlation check is `recipient`.
fn decode_source_evidence(
    direction: Direction,
    proxy_address: Address,
    logs: &[Log],
    destination_recipient: Address,
) -> Result<SourceEvidence> {
    if let Some(modern) =
        decode_modern_source_event(direction, proxy_address, logs, destination_recipient)?
    {
        return Ok(SourceEvidence::Modern(modern));
    }

    let legacy_topic = match direction {
        Direction::EthToGno => source_events::legacy::UserRequestForAffirmation::SIGNATURE_HASH,
        Direction::GnoToEth => source_events::legacy::UserRequestForSignature::SIGNATURE_HASH,
    };
    let matching = logs
        .iter()
        .filter(|log| log.address() == proxy_address && log.topic0() == Some(&legacy_topic))
        .collect::<Vec<_>>();
    ensure!(
        matching.len() <= 1,
        "multiple matching legacy xDai source events: at most one transfer per source \
         transaction is supported"
    );

    let decoded = matching
        .first()
        .map(|log| -> Result<LegacySourceEvent> {
            let event = match direction {
                Direction::EthToGno => {
                    let event =
                        source_events::legacy::UserRequestForAffirmation::decode_log_validate(
                            &log.inner,
                        )
                        .context("malformed legacy UserRequestForAffirmation")?;
                    LegacySourceEvent {
                        recipient: event.data.recipient,
                        value: event.data.value,
                    }
                }
                Direction::GnoToEth => {
                    let event =
                        source_events::legacy::UserRequestForSignature::decode_log_validate(
                            &log.inner,
                        )
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

    Ok(match decoded {
        Some(legacy) => SourceEvidence::Legacy(legacy),
        None => SourceEvidence::Missing,
    })
}

/// The modern-grammar half of [`decode_source_evidence`]. Returns `Ok(None)`
/// when no modern event is present at all (the caller then falls back to the
/// legacy check), and a hard error for a structurally invalid receipt (more
/// than one modern event, a malformed one, or a recipient mismatch) -- these
/// never silently degrade to the raw-hash path, because doing so could mint a
/// second logical transfer for a source transaction that already has one.
fn decode_modern_source_event(
    direction: Direction,
    proxy_address: Address,
    logs: &[Log],
    destination_recipient: Address,
) -> Result<Option<ModernSourceEvent>> {
    match direction {
        Direction::EthToGno => {
            let modern_topic =
                source_events::modern_affirmation::UserRequestForAffirmation::SIGNATURE_HASH;
            let matching = logs
                .iter()
                .filter(|log| log.address() == proxy_address && log.topic0() == Some(&modern_topic))
                .collect::<Vec<_>>();
            ensure!(
                matching.len() <= 1,
                "multiple modern xDai UserRequestForAffirmation events in one source \
                 transaction: at most one transfer per source transaction is supported"
            );
            let Some(log) = matching.first() else {
                return Ok(None);
            };
            let event =
                source_events::modern_affirmation::UserRequestForAffirmation::decode_log_validate(
                    &log.inner,
                )
                .context("malformed modern UserRequestForAffirmation")?;
            ensure!(
                event.data.recipient == destination_recipient,
                "modern xDai source recipient mismatch"
            );
            let nonce = U256::from_be_slice(event.data.nonce.as_slice());
            MessageIdentity::source(nonce).context("modern xDai source nonce out of range")?;
            Ok(Some(ModernSourceEvent {
                recipient: event.data.recipient,
                value: event.data.value,
                nonce,
                token: None,
            }))
        }
        Direction::GnoToEth => {
            let v6_topic =
                source_events::modern_signature_v6::UserRequestForSignature::SIGNATURE_HASH;
            let v7_topic =
                source_events::modern_signature_v7::UserRequestForSignature::SIGNATURE_HASH;
            let matching = logs
                .iter()
                .filter(|log| {
                    log.address() == proxy_address
                        && log
                            .topic0()
                            .is_some_and(|topic| *topic == v6_topic || *topic == v7_topic)
                })
                .collect::<Vec<_>>();
            ensure!(
                matching.len() <= 1,
                "multiple modern xDai UserRequestForSignature events in one source \
                 transaction: at most one transfer per source transaction is supported"
            );
            let Some(log) = matching.first() else {
                return Ok(None);
            };
            let topic = *log.topic0().expect("filtered by topic0 above");
            let (recipient, value, nonce, token) = if topic == v7_topic {
                let event =
                    source_events::modern_signature_v7::UserRequestForSignature::decode_log_validate(
                        &log.inner,
                    )
                    .context("malformed modern UserRequestForSignature (v7)")?;
                (
                    event.data.recipient,
                    event.data.value,
                    event.data.nonce,
                    Some(event.data.token),
                )
            } else {
                let event =
                    source_events::modern_signature_v6::UserRequestForSignature::decode_log_validate(
                        &log.inner,
                    )
                    .context("malformed modern UserRequestForSignature (v6)")?;
                (
                    event.data.recipient,
                    event.data.value,
                    event.data.nonce,
                    None,
                )
            };
            ensure!(
                recipient == destination_recipient,
                "modern xDai source recipient mismatch"
            );
            let nonce = U256::from_be_slice(nonce.as_slice());
            MessageIdentity::source(nonce).context("modern xDai source nonce out of range")?;
            Ok(Some(ModernSourceEvent {
                recipient,
                value,
                nonce,
                token,
            }))
        }
    }
}

/// Canonical identity resolved for one destination completion, plus receipt
/// evidence when reconstruction ran.
enum CanonicalCompletionIdentity {
    Nonce {
        nonce: U256,
        /// `Some` only when this nonce came from receipt reconstruction
        /// (a hash-keyed observation whose receipt carried a modern source
        /// event) -- a directly nonce-keyed observation makes no RPC call at
        /// all and carries no evidence to fill source fields from.
        evidence: Option<NonceEvidence>,
    },
    SourceTransactionHash(ReconstructedSource),
}

/// Receipt-derived facts for a [`CanonicalCompletionIdentity::Nonce`] whose
/// nonce came from reconstruction, bundled together since they are always
/// consumed together (see the `handle_*` completion handlers).
struct NonceEvidence {
    event: ModernSourceEvent,
    facts: SourceFacts,
}

/// `match identity { Nonce => None RPC, SourceTransactionHash => reconstruct }`
/// -- the whole reason nonce-keyed completions make zero additional requests.
/// Only a hash-keyed observation ever calls [`reconstruct_source`].
async fn resolve_canonical_completion_identity(
    ctx: &EventContext<'_>,
    direction: Direction,
    observed_identity: MessageIdentity,
    destination_recipient: Address,
) -> Result<CanonicalCompletionIdentity> {
    match observed_identity {
        MessageIdentity::Nonce(nonce) => Ok(CanonicalCompletionIdentity::Nonce {
            nonce,
            evidence: None,
        }),
        MessageIdentity::SourceTransactionHash(source_hash) => {
            match reconstruct_source(ctx, direction, source_hash, destination_recipient).await? {
                ReconstructedSourceIdentity::Nonce {
                    nonce,
                    event,
                    facts,
                } => Ok(CanonicalCompletionIdentity::Nonce {
                    nonce,
                    evidence: Some(NonceEvidence { event, facts }),
                }),
                ReconstructedSourceIdentity::SourceTransactionHash(source) => {
                    Ok(CanonicalCompletionIdentity::SourceTransactionHash(source))
                }
            }
        }
    }
}

/// Resolves `source_asset` for a receipt-derived modern
/// `UserRequestForAffirmation` exactly the way the stream path resolves it
/// (`handle_user_request_for_affirmation`): the grammar window covering the
/// *source* block, through the same `AbiRegistry` the stream uses --
/// `ctx.abi_registry` is shared across both chains of the bridge
/// (`AbiRegistry::from_chains` puts every configured contract in one `inner`),
/// so `(chain, address, block)` resolves to the same window on both paths by
/// construction, not by coincidence.
///
/// `legacy_ethereum_asset` is a fallback only, for the case where no
/// configured window covers `source_block` (`NotConfigured` / `WrongVersion`
/// -- the block is below the earliest configured Foreign window). That case
/// is practically unreachable for a genuinely modern event, but the branch
/// must be total rather than panic. `legacy_ethereum_asset`'s own doc lists
/// this as its second caller, alongside its original one in
/// `consolidation.rs`'s legacy reconstruction path.
fn resolve_modern_source_asset(
    ctx: &EventContext<'_>,
    foreign_chain_id: i64,
    foreign_proxy_address: Address,
    source_block: u64,
) -> Result<Address> {
    let modern_topic = source_events::modern_affirmation::UserRequestForAffirmation::SIGNATURE_HASH;
    match ctx.abi_registry.resolve_log(
        foreign_chain_id,
        foreign_proxy_address,
        &modern_topic,
        source_block,
    ) {
        LogResolution::Matched(_, kind) => {
            grammar_for(foreign_chain_id, XDaiSide::Foreign, kind.version)?
                .source_asset
                .context("xDai Foreign grammar has no source_asset")
        }
        LogResolution::NotConfigured | LogResolution::WrongVersion => {
            legacy_ethereum_asset(foreign_chain_id, Direction::EthToGno, source_block)
        }
    }
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
        primitives::{Bytes, LogData, keccak256},
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

    fn raw_log(address: Address, topics: Vec<B256>, data: Vec<u8>) -> Log {
        rpc_log(address, LogData::new_unchecked(topics, Bytes::from(data)))
    }

    fn word_address(a: Address) -> [u8; 32] {
        let mut buf = [0u8; 32];
        buf[12..].copy_from_slice(a.as_slice());
        buf
    }

    fn word_u256(v: U256) -> [u8; 32] {
        v.to_be_bytes::<32>()
    }

    /// Verified real on-chain `topic0`s (`cast keccak`, recorded in the task's
    /// preconditions) for the genuine two-argument source-request events.
    /// Fixtures below parse these literals directly rather than reading
    /// `source_events::legacy::*::SIGNATURE_HASH` off the same declaration the
    /// decoder itself uses -- that self-referential shape is exactly how the
    /// dead `Legacy*`-named topic0 bug survived undetected (see
    /// `.memory-bank/gotchas.md`).
    const REAL_USER_REQUEST_FOR_AFFIRMATION_TWO_ARG_TOPIC0: &str =
        "0x1d491a427d1f8cc0d447496f300fac39f7306122481d8e663451eb268274146b";
    const REAL_USER_REQUEST_FOR_SIGNATURE_TWO_ARG_TOPIC0: &str =
        "0x127650bcfb0ba017401abe4931453a405140a8fd36fece67bae2db174d3fdd63";

    fn topic0(hex: &str) -> B256 {
        hex.parse().expect("valid topic0 hex literal")
    }

    /// Manually built two-argument `(recipient, value)` log data/topic, using
    /// the literal on-chain topic0 rather than any `sol!`-generated constant.
    fn legacy_two_arg_log(
        address: Address,
        direction: Direction,
        recipient: Address,
        value: U256,
    ) -> Log {
        let topic = match direction {
            Direction::EthToGno => topic0(REAL_USER_REQUEST_FOR_AFFIRMATION_TWO_ARG_TOPIC0),
            Direction::GnoToEth => topic0(REAL_USER_REQUEST_FOR_SIGNATURE_TWO_ARG_TOPIC0),
        };
        let mut data = Vec::with_capacity(64);
        data.extend_from_slice(&word_address(recipient));
        data.extend_from_slice(&word_u256(value));
        raw_log(address, vec![topic], data)
    }

    fn modern_affirmation_log(
        address: Address,
        recipient: Address,
        value: U256,
        nonce: U256,
    ) -> Log {
        rpc_log(
            address,
            source_events::modern_affirmation::UserRequestForAffirmation {
                recipient,
                value,
                nonce: B256::from(nonce.to_be_bytes::<32>()),
            }
            .encode_log_data(),
        )
    }

    fn modern_signature_v6_log(
        address: Address,
        recipient: Address,
        value: U256,
        nonce: U256,
    ) -> Log {
        rpc_log(
            address,
            source_events::modern_signature_v6::UserRequestForSignature {
                recipient,
                value,
                nonce: B256::from(nonce.to_be_bytes::<32>()),
            }
            .encode_log_data(),
        )
    }

    fn modern_signature_v7_log(
        address: Address,
        recipient: Address,
        value: U256,
        nonce: U256,
        token: Address,
    ) -> Log {
        rpc_log(
            address,
            source_events::modern_signature_v7::UserRequestForSignature {
                recipient,
                value,
                nonce: B256::from(nonce.to_be_bytes::<32>()),
                token,
            }
            .encode_log_data(),
        )
    }

    #[test]
    fn source_evidence_decodes_real_legacy_topic0_in_both_directions() {
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);

        let affirmation_evidence = decode_source_evidence(
            Direction::EthToGno,
            proxy,
            &[legacy_two_arg_log(
                proxy,
                Direction::EthToGno,
                recipient,
                U256::from(1_001u64),
            )],
            recipient,
        )
        .unwrap();
        assert!(matches!(
            affirmation_evidence,
            SourceEvidence::Legacy(LegacySourceEvent { recipient: r, value })
                if r == recipient && value == U256::from(1_001u64)
        ));

        let signature_evidence = decode_source_evidence(
            Direction::GnoToEth,
            proxy,
            &[legacy_two_arg_log(
                proxy,
                Direction::GnoToEth,
                recipient,
                U256::from(2_002u64),
            )],
            recipient,
        )
        .unwrap();
        assert!(matches!(
            signature_evidence,
            SourceEvidence::Legacy(LegacySourceEvent { recipient: r, value })
                if r == recipient && value == U256::from(2_002u64)
        ));
    }

    #[test]
    fn source_evidence_ignores_another_proxy_and_falls_back_to_missing() {
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);
        let log = legacy_two_arg_log(
            Address::repeat_byte(3),
            Direction::EthToGno,
            recipient,
            U256::ONE,
        );
        assert!(matches!(
            decode_source_evidence(
                Direction::EthToGno,
                proxy,
                std::slice::from_ref(&log),
                recipient,
            )
            .unwrap(),
            SourceEvidence::Missing
        ));
        assert!(matches!(
            decode_source_evidence(Direction::GnoToEth, proxy, &[log], recipient).unwrap(),
            SourceEvidence::Missing
        ));
    }

    #[test]
    fn plain_transfer_receipt_without_proxy_logs_is_missing() {
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);
        assert!(matches!(
            decode_source_evidence(Direction::EthToGno, proxy, &[], recipient).unwrap(),
            SourceEvidence::Missing
        ));
    }

    #[test]
    fn legacy_source_event_rejects_malformed_multiple_and_recipient_mismatch() {
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);
        let valid = legacy_two_arg_log(proxy, Direction::GnoToEth, recipient, U256::ONE);
        let malformed = raw_log(
            proxy,
            vec![topic0(REAL_USER_REQUEST_FOR_SIGNATURE_TWO_ARG_TOPIC0)],
            vec![0u8; 1],
        );
        assert!(
            decode_source_evidence(Direction::GnoToEth, proxy, &[malformed], recipient).is_err()
        );
        assert!(
            decode_source_evidence(
                Direction::GnoToEth,
                proxy,
                &[valid.clone(), valid.clone()],
                recipient,
            )
            .is_err()
        );
        assert!(
            decode_source_evidence(
                Direction::GnoToEth,
                proxy,
                &[valid],
                Address::repeat_byte(4),
            )
            .is_err()
        );
    }

    /// A modern receipt must resolve to `Modern`, never fall through to
    /// `Legacy` or `Missing` -- letting it fall through would mint a second
    /// logical transfer under a raw-hash identity for a transaction that
    /// already has a nonce-keyed one.
    #[test]
    fn modern_source_request_never_becomes_legacy_or_missing() {
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);

        let eth_to_gno = decode_source_evidence(
            Direction::EthToGno,
            proxy,
            &[modern_affirmation_log(
                proxy,
                recipient,
                U256::from(1_000u64),
                U256::from(7u64),
            )],
            recipient,
        )
        .unwrap();
        assert!(matches!(eth_to_gno, SourceEvidence::Modern(_)));

        let gno_to_eth = decode_source_evidence(
            Direction::GnoToEth,
            proxy,
            &[modern_signature_v6_log(
                proxy,
                recipient,
                U256::from(2_000u64),
                U256::from(8u64),
            )],
            recipient,
        )
        .unwrap();
        assert!(matches!(gno_to_eth, SourceEvidence::Modern(_)));
    }

    #[test]
    fn modern_source_evidence_rejects_multiple_events_in_one_transaction() {
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);
        let logs = vec![
            modern_affirmation_log(proxy, recipient, U256::from(1u64), U256::from(1u64)),
            modern_affirmation_log(proxy, recipient, U256::from(2u64), U256::from(2u64)),
        ];
        let err = decode_source_evidence(Direction::EthToGno, proxy, &logs, recipient)
            .expect_err("two modern events in one source transaction must be a hard error");
        assert!(
            err.to_string()
                .contains("one transfer per source transaction"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn modern_source_evidence_rejects_recipient_mismatch() {
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);
        let log = modern_affirmation_log(proxy, recipient, U256::from(1_000u64), U256::from(9u64));
        assert!(
            decode_source_evidence(Direction::EthToGno, proxy, &[log], Address::repeat_byte(9),)
                .is_err()
        );
    }

    /// Both Gno->Eth modern grammars (Home v6, three-argument; Home v7,
    /// four-argument) must be recognized, and only the four-argument one
    /// carries `token`.
    #[test]
    fn modern_gno_to_eth_evidence_recognizes_both_v6_and_v7_grammars() {
        let proxy = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);

        let v6 = decode_source_evidence(
            Direction::GnoToEth,
            proxy,
            &[modern_signature_v6_log(
                proxy,
                recipient,
                U256::from(1_000u64),
                U256::from(10u64),
            )],
            recipient,
        )
        .unwrap();
        let SourceEvidence::Modern(v6_event) = v6 else {
            panic!("expected Modern evidence for the v6 grammar");
        };
        assert_eq!(v6_event.nonce, U256::from(10u64));
        assert_eq!(v6_event.token, None);

        let token = Address::repeat_byte(0xAB);
        let v7 = decode_source_evidence(
            Direction::GnoToEth,
            proxy,
            &[modern_signature_v7_log(
                proxy,
                recipient,
                U256::from(2_000u64),
                U256::from(11u64),
                token,
            )],
            recipient,
        )
        .unwrap();
        let SourceEvidence::Modern(v7_event) = v7 else {
            panic!("expected Modern evidence for the v7 grammar");
        };
        assert_eq!(v7_event.nonce, U256::from(11u64));
        assert_eq!(v7_event.token, Some(token));
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
        let mut source_log = if recognized_source_event {
            legacy_two_arg_log(proxy, Direction::GnoToEth, recipient, U256::from(49_240u64))
        } else {
            // A historical event topic genuinely outside the decoder's
            // supported grammar -- unlike the pre-fix fixture, this is no
            // longer the real two-argument `UserRequestForSignature` topic
            // (which the fix now recognizes), so it must be some other,
            // truly unrecognized, signature.
            raw_log(
                proxy,
                vec![keccak256("SomeUnknownXDaiSourceEvent(address,uint256)")],
                {
                    let mut data = Vec::with_capacity(64);
                    data.extend_from_slice(&word_address(recipient));
                    data.extend_from_slice(&word_u256(U256::from(49_240u64)));
                    data
                },
            )
        };
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
        let reconstructed = fetch_reconstructed_source(
            &counterpart,
            1,
            Direction::GnoToEth,
            source_hash,
            recipient,
        )
        .await
        .unwrap();
        let ReconstructedSourceIdentity::SourceTransactionHash(reconstructed) = reconstructed
        else {
            panic!("a legacy/missing receipt must resolve to SourceTransactionHash identity");
        };

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
