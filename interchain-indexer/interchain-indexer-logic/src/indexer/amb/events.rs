use std::{collections::HashMap, sync::Arc};

use alloy::{
    dyn_abi::{DynSolValue, EventExt},
    primitives::{Address, B256, keccak256},
    rpc::types::{Block, Log},
};
use anyhow::{Context, Result, bail, ensure};
use dashmap::DashMap;

use crate::message_buffer::{Key, MessageBuffer};

use super::{
    abi::{AbiRegistry, ContractKind},
    header::parse_amb_header,
    settings::AmbIndexerSettings,
    types::{
        AnnotatedEvent, CollectedSignaturesEvent, DestinationExecution, DestinationExecutionEvent,
        DestinationTransferDetails, Direction, Message, SourceRequest, SourceRequestEvent,
        SourceTransferDetails, ValidatorConfirmation,
    },
    version::AmbSide,
};

pub(super) struct EventContext<'a> {
    pub(super) bridge_id: i32,
    pub(super) chain_id: i64,
    pub(super) abi_registry: &'a AbiRegistry,
    pub(super) buffer: &'a Arc<MessageBuffer<Message>>,
    pub(super) message_hash_lookup: &'a Arc<DashMap<B256, Key>>,
    pub(super) pending_message_hash_events: &'a Arc<DashMap<B256, PendingMessageHashEvents>>,
    pub(super) settings: &'a AmbIndexerSettings,
}

/// AMB-local wrapper around `MessageBuffer::alter` that stamps the current
/// `clock_skew_tolerance` onto the entry before applying the mutation.
///
/// All AMB mutations must go through this helper so that `consolidate` — which
/// only ever runs on hot entries reached via an `alter` — always sees a current
/// tolerance, even for entries restored from the cold tier (the tolerance is
/// `#[serde(skip)]` and therefore not persisted).
async fn alter_amb<F>(
    ctx: &EventContext<'_>,
    key: Key,
    chain_id: u64,
    block_number: u64,
    mutator: F,
) -> Result<()>
where
    F: FnOnce(&mut Message) -> Result<()>,
{
    ctx.buffer
        .alter(key, chain_id, block_number, |message| {
            message.clock_skew_tolerance = ctx.settings.clock_skew_tolerance;
            mutator(message)
        })
        .await
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

pub(super) async fn dispatch_transaction(
    ctx: &EventContext<'_>,
    receipt_logs: &[Log],
    block: &Block,
    transaction_from: Address,
) -> Result<()> {
    let block_timestamp = chrono::DateTime::from_timestamp(block.header.timestamp as i64, 0)
        .map(|dt| dt.naive_utc())
        .context("invalid block timestamp")?;

    for log in receipt_logs {
        let Some(topic) = log.topic0() else {
            continue;
        };
        let Some((event, kind)) =
            ctx.abi_registry
                .event_for_log(ctx.chain_id, log.address(), topic)
        else {
            continue;
        };

        tracing::trace!(
            bridge_id = ctx.bridge_id,
            chain_id = ctx.chain_id,
            tx_hash = ?log.transaction_hash,
            log_index = ?log.log_index,
            address = %log.address(),
            event_name = %event.name,
            "AMB diag: matched configured event"
        );

        let result = match event.name.as_str() {
            "UserRequestForAffirmation" => {
                handle_source_request(
                    ctx,
                    event,
                    log,
                    receipt_logs,
                    block_timestamp,
                    Direction::ForeignToHome,
                    transaction_from,
                )
                .await
            }
            "UserRequestForSignature" => {
                handle_source_request(
                    ctx,
                    event,
                    log,
                    receipt_logs,
                    block_timestamp,
                    Direction::HomeToForeign,
                    transaction_from,
                )
                .await
            }
            "SignedForAffirmation" | "SignedForUserRequest" => {
                handle_validator_confirmation(ctx, event, log, block_timestamp).await
            }
            "CollectedSignatures" => {
                handle_collected_signatures(ctx, event, log, block_timestamp).await
            }
            "AffirmationCompleted" => {
                let out = handle_destination_execution(
                    ctx,
                    event,
                    log,
                    receipt_logs,
                    block_timestamp,
                    DestinationKind::Affirmation,
                )
                .await;
                out.map(|_| ())
            }
            "RelayedMessage" => {
                let out = handle_destination_execution(
                    ctx,
                    event,
                    log,
                    receipt_logs,
                    block_timestamp,
                    DestinationKind::Relayed,
                )
                .await;
                out.map(|_| ())
            }
            _ => Ok(()),
        };

        if let Err(err) = result {
            tracing::error!(
                bridge_id = ctx.bridge_id,
                chain_id = ctx.chain_id,
                tx_hash = ?log.transaction_hash,
                log_index = ?log.log_index,
                event_name = event.name,
                err = ?err,
                "failed to process AMB event"
            );
        } else if matches!(kind, ContractKind::AmbProxy { .. }) {
            tracing::debug!(
                bridge_id = ctx.bridge_id,
                chain_id = ctx.chain_id,
                tx_hash = ?log.transaction_hash,
                log_index = ?log.log_index,
                event_name = event.name,
                "processed AMB event"
            );
        }
    }

    Ok(())
}

async fn handle_source_request(
    ctx: &EventContext<'_>,
    event: &alloy::json_abi::Event,
    log: &Log,
    receipt_logs: &[Log],
    block_timestamp: chrono::NaiveDateTime,
    direction: Direction,
    transaction_from: Address,
) -> Result<()> {
    let decoded = event.decode_log(log.data())?;
    let message_id = expect_b256(decoded.indexed.first(), "messageId")?;
    let encoded_data = expect_bytes(decoded.body.first(), "encodedData")?.to_vec();
    let (
        _,
        ContractKind::AmbProxy {
            side,
            header_layout,
        },
    ) = ctx
        .abi_registry
        .event_for_log(
            ctx.chain_id,
            log.address(),
            log.topic0().expect("topic exists"),
        )
        .context("source event contract missing from registry")?
    else {
        bail!("source request was not emitted by AMB proxy");
    };
    match (direction, side) {
        (Direction::ForeignToHome, AmbSide::Foreign)
        | (Direction::HomeToForeign, AmbSide::Home) => {}
        _ => bail!("source request emitted on unexpected AMB side"),
    }

    let header = parse_amb_header(&encoded_data, header_layout)?;
    let expected_destination_chain_id = ctx.abi_registry.counterpart_chain_id(side)?;
    ensure!(
        header.source_chain_id == ctx.chain_id,
        "AMB header source_chain_id {} does not match emitting chain {}",
        header.source_chain_id,
        ctx.chain_id,
    );
    ensure!(
        header.destination_chain_id == expected_destination_chain_id,
        "AMB header destination_chain_id {} does not match configured counterpart {}",
        header.destination_chain_id,
        expected_destination_chain_id,
    );
    ensure!(
        header.source_chain_id != header.destination_chain_id,
        "AMB header has source_chain_id == destination_chain_id ({})",
        header.source_chain_id,
    );
    let source_chain_id = header.source_chain_id;
    let block_number = log.block_number.context("missing block number")?;
    let destination_chain_id = header.destination_chain_id;
    let application_calldata = encoded_data[header.payload_offset..].to_vec();
    let key = key_from_message_id(&message_id, ctx.bridge_id)?;
    let message_hash = keccak256(&encoded_data);
    let source_identity = (header.sender, header.executor);
    ctx.message_hash_lookup.insert(message_hash, key);

    tracing::trace!(
        bridge_id = ctx.bridge_id,
        chain_id = ctx.chain_id,
        ?direction,
        message_id = %message_id,
        buffer_key = key.message_id,
        message_hash = %message_hash,
        source_chain_id,
        destination_chain_id,
        transaction_from = %transaction_from,
        tx_hash = ?log.transaction_hash,
        "AMB diag: source request -> buffer key"
    );

    let annotated = AnnotatedEvent {
        event: SourceRequestEvent {
            message_id,
            encoded_data,
            application_calldata,
            header: header.into(),
            transaction_from,
        },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
        source_chain_id,
        destination_chain_id,
    };

    let mut displaced_by_existing_destination = false;
    alter_amb(
        ctx,
        key,
        ctx.chain_id as u64,
        annotated.block_number as u64,
        |message| {
            displaced_by_existing_destination = message
                .destination_execution
                .as_ref()
                .map(|destination| {
                    let destination = destination.event();
                    (destination.event.sender, destination.event.executor) != source_identity
                })
                .unwrap_or(false);
            message.direction = Some(direction);
            message.source_request = Some(match direction {
                Direction::ForeignToHome => SourceRequest::Affirmation(annotated),
                Direction::HomeToForeign => SourceRequest::Signature(annotated),
            });
            Ok(())
        },
    )
    .await?;

    if let Some(source_transfer) = find_tokens_bridging_initiated(ctx, receipt_logs, &message_id) {
        alter_amb(ctx, key, ctx.chain_id as u64, block_number, |message| {
            message.source_transfer = Some(source_transfer);
            Ok(())
        })
        .await?;
    }

    if displaced_by_existing_destination
        && ctx
            .message_hash_lookup
            .remove_if(&message_hash, |_, canonical_key| *canonical_key == key)
            .is_some()
    {
        tracing::warn!(
            bridge_id = ctx.bridge_id,
            chain_id = ctx.chain_id,
            tx_hash = ?log.transaction_hash,
            message_id = %message_id,
            message_hash = %message_hash,
            "dropped displaced source messageHash from lookup after AMB messageId collision"
        );
    }

    if is_canonical_message_hash_lookup(ctx.message_hash_lookup, message_hash, key) {
        drain_pending_message_hash_events(ctx, message_hash, key).await
    } else {
        tracing::debug!(
            bridge_id = ctx.bridge_id,
            chain_id = ctx.chain_id,
            tx_hash = ?log.transaction_hash,
            message_id = %message_id,
            message_hash = %message_hash,
            "skipped queued AMB message-hash events for non-canonical source request"
        );
        Ok(())
    }
}

/// Scan the source transaction's receipt for the mediator's
/// `TokensBridgingInitiated(address indexed token, address indexed sender, uint256 value, bytes32 indexed messageId)`
/// event matching `message_id`. Returns the source-side token, sender, and amount.
fn find_tokens_bridging_initiated(
    ctx: &EventContext<'_>,
    receipt_logs: &[Log],
    message_id: &B256,
) -> Option<SourceTransferDetails> {
    for log in receipt_logs {
        let Some(topic) = log.topic0() else {
            continue;
        };
        let Some((event, kind)) =
            ctx.abi_registry
                .event_for_log(ctx.chain_id, log.address(), topic)
        else {
            continue;
        };
        if !matches!(kind, ContractKind::OmnibridgeMediator)
            || event.name != "TokensBridgingInitiated"
        {
            continue;
        }
        let Ok(decoded) = event.decode_log(log.data()) else {
            continue;
        };
        let Some(DynSolValue::Address(token)) = decoded.indexed.first() else {
            continue;
        };
        let Some(DynSolValue::Address(sender)) = decoded.indexed.get(1) else {
            continue;
        };
        let Some(DynSolValue::FixedBytes(event_message_id, 32)) = decoded.indexed.get(2) else {
            continue;
        };
        if event_message_id != message_id {
            continue;
        }
        let Some(DynSolValue::Uint(amount, _)) = decoded.body.first() else {
            continue;
        };
        return Some(SourceTransferDetails {
            token: *token,
            sender: *sender,
            amount: *amount,
        });
    }
    None
}

/// Scan the destination transaction's receipt for the mediator's
/// `TokensBridged(address indexed token, address indexed recipient, uint256 value, bytes32 indexed messageId)`
/// event matching `message_id`. Returns the destination-side token, recipient,
/// and amount.
///
/// Matched on (registered Omnibridge mediator on `chain_id`) + event name +
/// `message_id`, mirroring [`find_tokens_bridging_initiated`]. The emitting
/// address is deliberately **not** required to equal the AMB execution
/// `executor`: when a message is routed through a contract that forwards to the
/// mediator (e.g. a Safe), the recorded `executor` is that recipient contract,
/// not the mediator that emits `TokensBridged`. `message_id` is unique, so it is
/// a sufficient and reliable key.
fn find_tokens_bridged(
    abi_registry: &AbiRegistry,
    chain_id: i64,
    receipt_logs: &[Log],
    message_id: &B256,
) -> Option<DestinationTransferDetails> {
    for log in receipt_logs {
        let Some(topic) = log.topic0() else {
            continue;
        };
        let Some((event, kind)) = abi_registry.event_for_log(chain_id, log.address(), topic) else {
            continue;
        };
        if !matches!(kind, ContractKind::OmnibridgeMediator) || event.name != "TokensBridged" {
            continue;
        }
        let Ok(decoded) = event.decode_log(log.data()) else {
            continue;
        };
        let Some(DynSolValue::Address(token)) = decoded.indexed.first() else {
            continue;
        };
        let Some(DynSolValue::Address(recipient)) = decoded.indexed.get(1) else {
            continue;
        };
        let Some(DynSolValue::FixedBytes(event_message_id, 32)) = decoded.indexed.get(2) else {
            continue;
        };
        if event_message_id != message_id {
            continue;
        }
        let Some(DynSolValue::Uint(amount, _)) = decoded.body.first() else {
            continue;
        };
        return Some(DestinationTransferDetails {
            token: *token,
            recipient: *recipient,
            amount: *amount,
        });
    }
    None
}

async fn handle_validator_confirmation(
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
                "queued AMB validator confirmation until source request is processed"
            );
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
    let side = ctx.abi_registry.side_for_chain(ctx.chain_id)?;
    let destination_chain_id = ctx.abi_registry.counterpart_chain_id(side)?;
    let annotated = AnnotatedEvent {
        event: CollectedSignaturesEvent {
            authority_responsible_for_relay: authority,
            message_hash,
            count,
        },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
        source_chain_id: ctx.chain_id,
        destination_chain_id,
    };

    match ctx.message_hash_lookup.get(&message_hash).map(|key| *key) {
        Some(key) => {
            tracing::trace!(
                bridge_id = ctx.bridge_id,
                chain_id = ctx.chain_id,
                buffer_key = key.message_id,
                message_hash = %message_hash,
                tx_hash = ?log.transaction_hash,
                "AMB diag: collected signatures -> buffer key (source known)"
            );
            apply_collected_signatures(ctx, key, ctx.chain_id as u64, annotated).await
        }
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
                "queued AMB collected-signatures event until source request is processed"
            );
            Ok(())
        }
    }
}

async fn drain_pending_message_hash_events(
    ctx: &EventContext<'_>,
    message_hash: B256,
    key: Key,
) -> Result<()> {
    let Some((_, pending)) = ctx.pending_message_hash_events.remove(&message_hash) else {
        return Ok(());
    };
    let confirmation_count = pending.validator_confirmations.len();
    let has_signatures_collected = pending.signatures_collected.is_some();

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

    tracing::debug!(
        bridge_id = ctx.bridge_id,
        chain_id = ctx.chain_id,
        message_hash = %message_hash,
        confirmation_count,
        has_signatures_collected,
        "drained queued AMB message-hash events"
    );

    Ok(())
}

fn is_canonical_message_hash_lookup(
    message_hash_lookup: &DashMap<B256, Key>,
    message_hash: B256,
    key: Key,
) -> bool {
    message_hash_lookup
        .get(&message_hash)
        .is_some_and(|canonical_key| *canonical_key == key)
}

async fn apply_validator_confirmation(
    ctx: &EventContext<'_>,
    key: Key,
    chain_id: u64,
    confirmation: ValidatorConfirmation,
) -> Result<()> {
    let block_number = confirmation.block_number;
    alter_amb(ctx, key, chain_id, block_number, |message| {
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
    alter_amb(ctx, key, chain_id, block_number, |message| {
        message.signatures_collected = Some(annotated);
        Ok(())
    })
    .await
}

#[derive(Clone, Copy)]
enum DestinationKind {
    Affirmation,
    Relayed,
}

async fn handle_destination_execution(
    ctx: &EventContext<'_>,
    event: &alloy::json_abi::Event,
    log: &Log,
    receipt_logs: &[Log],
    block_timestamp: chrono::NaiveDateTime,
    kind: DestinationKind,
) -> Result<Option<Key>> {
    let decoded = event.decode_log(log.data())?;
    let sender = expect_address(decoded.indexed.first(), "sender")?;
    let executor = expect_address(decoded.indexed.get(1), "executor")?;
    let message_id = expect_b256(decoded.indexed.get(2), "messageId")?;
    let status = expect_bool(decoded.body.first(), "status")?;
    let (_, contract_kind) = ctx
        .abi_registry
        .event_for_log(
            ctx.chain_id,
            log.address(),
            log.topic0().expect("topic exists"),
        )
        .context("destination event contract missing from registry")?;
    let ContractKind::AmbProxy { side, .. } = contract_kind else {
        bail!("destination execution was not emitted by AMB proxy");
    };
    let source_chain_id = ctx.abi_registry.counterpart_chain_id(side)?;
    let key = key_from_message_id(&message_id, ctx.bridge_id)?;
    let block_number = log.block_number.context("missing block number")?;
    let execution = DestinationExecutionEvent {
        sender,
        executor,
        message_id,
        status,
    };
    let annotated = AnnotatedEvent {
        event: execution.clone(),
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
        source_chain_id,
        destination_chain_id: ctx.chain_id,
    };
    let destination_transfer =
        find_tokens_bridged(ctx.abi_registry, ctx.chain_id, receipt_logs, &message_id);

    tracing::trace!(
        bridge_id = ctx.bridge_id,
        chain_id = ctx.chain_id,
        message_id = %message_id,
        buffer_key = key.message_id,
        counterpart_source_chain_id = source_chain_id,
        sender = %sender,
        executor = %executor,
        status,
        tx_hash = ?log.transaction_hash,
        "AMB diag: destination execution -> buffer key"
    );

    // `messageId` collision safeguards (the authoritative split happens in
    // `consolidate`, which compares both sides and the timestamps):
    // - if an existing source request carries a different `(sender, executor)`
    //   header, the source body is being displaced — schedule removal of its
    //   `messageHash` from the lookup so stray validator confirmations do not
    //   attach to the executed entry, and skip payload decode of the mixed body;
    // - if a *second* destination execution conflicts with the one already held,
    //   capture it in `displaced` instead of overwriting the canonical one.
    let new_identity = (sender, executor);
    let mut displaced_source_hash: Option<B256> = None;
    let mut diag_existing_source_identity: Option<(Address, Address)> = None;
    let mut diag_pushed_as_displaced = false;
    alter_amb(ctx, key, ctx.chain_id as u64, block_number, |message| {
        let new_execution = match kind {
            DestinationKind::Affirmation => DestinationExecution::Affirmation(annotated),
            DestinationKind::Relayed => DestinationExecution::Relayed(annotated),
        };

        if let Some(source) = &message.source_request {
            let header = &source.event().event.header;
            diag_existing_source_identity = Some((header.sender, header.executor));
            if (header.sender, header.executor) != new_identity {
                displaced_source_hash = Some(keccak256(&source.event().event.encoded_data));
            }
        }

        let conflicts_existing_destination = message
            .destination_execution
            .as_ref()
            .map(|existing| {
                let existing = existing.event();
                (existing.event.sender, existing.event.executor) != new_identity
            })
            .unwrap_or(false);
        if conflicts_existing_destination {
            message.displaced.push(new_execution);
            diag_pushed_as_displaced = true;
        } else {
            message.destination_execution = Some(new_execution);
            if let Some(destination_transfer) = destination_transfer {
                message.destination_transfer = Some(destination_transfer);
            }
        }

        Ok(())
    })
    .await?;

    tracing::trace!(
        bridge_id = ctx.bridge_id,
        chain_id = ctx.chain_id,
        message_id = %message_id,
        buffer_key = key.message_id,
        source_already_buffered = diag_existing_source_identity.is_some(),
        existing_source_identity = ?diag_existing_source_identity,
        new_identity = ?new_identity,
        pushed_as_displaced = diag_pushed_as_displaced,
        displaced_source = displaced_source_hash.is_some(),
        "AMB diag: destination execution applied to buffer entry"
    );

    if let Some(message_hash) = displaced_source_hash
        && ctx.message_hash_lookup.remove(&message_hash).is_some()
    {
        tracing::warn!(
            bridge_id = ctx.bridge_id,
            chain_id = ctx.chain_id,
            tx_hash = ?log.transaction_hash,
            message_id = %message_id,
            message_hash = %message_hash,
            "dropped displaced source messageHash from lookup after AMB messageId collision"
        );
    }

    Ok(Some(key))
}

// Derive an i64 buffer key from the full 32-byte AMB `messageId`.
//
// The raw AMB `messageId` is structured as
// `[4-byte version | 20-byte AMB proxy address | 8-byte nonce]`, so its last 8
// bytes are *just the nonce*. Home and Foreign proxies increment nonces
// independently, which means same-nonce messages from opposite sides collide
// on any key derived from those tail bytes. Hashing the full 32 bytes spreads
// the key uniformly over the i64 space.
fn key_from_message_id(message_id: &B256, bridge_id: i32) -> Result<Key> {
    let digest = keccak256(message_id.as_slice());
    let bytes: [u8; 8] = digest.as_slice()[..8].try_into()?;
    Ok(Key::new(
        i64::from_be_bytes(bytes),
        i16::try_from(bridge_id).context("bridge_id out of range")?,
    ))
}

fn expect_b256(value: Option<&DynSolValue>, name: &str) -> Result<B256> {
    match value {
        Some(DynSolValue::FixedBytes(value, 32)) => Ok(*value),
        other => bail!("expected bytes32 {name}, got {other:?}"),
    }
}

fn expect_bytes<'a>(value: Option<&'a DynSolValue>, name: &str) -> Result<&'a [u8]> {
    match value {
        Some(DynSolValue::Bytes(value)) => Ok(value),
        other => bail!("expected bytes {name}, got {other:?}"),
    }
}

fn expect_address(value: Option<&DynSolValue>, name: &str) -> Result<Address> {
    match value {
        Some(DynSolValue::Address(value)) => Ok(*value),
        other => bail!("expected address {name}, got {other:?}"),
    }
}

fn expect_bool(value: Option<&DynSolValue>, name: &str) -> Result<bool> {
    match value {
        Some(DynSolValue::Bool(value)) => Ok(*value),
        other => bail!("expected bool {name}, got {other:?}"),
    }
}

fn expect_uint(value: Option<&DynSolValue>, name: &str) -> Result<alloy::primitives::U256> {
    match value {
        Some(DynSolValue::Uint(value, _)) => Ok(*value),
        other => bail!("expected uint {name}, got {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use alloy::{
        json_abi::Event,
        primitives::{Address, B256, Bytes, LogData, U256, address, b256},
        rpc::types::Log,
    };
    use dashmap::DashMap;

    use super::{find_tokens_bridged, is_canonical_message_hash_lookup};
    use crate::{
        indexer::amb::abi::{AbiRegistry, ContractAbi, ContractKind},
        message_buffer::Key,
    };

    fn tokens_bridged_event() -> Event {
        serde_json::from_str(
            r#"{
                "anonymous": false,
                "inputs": [
                    {"indexed": true,  "name": "token",     "type": "address"},
                    {"indexed": true,  "name": "recipient", "type": "address"},
                    {"indexed": false, "name": "value",     "type": "uint256"},
                    {"indexed": true,  "name": "messageId", "type": "bytes32"}
                ],
                "name": "TokensBridged",
                "type": "event"
            }"#,
        )
        .expect("TokensBridged ABI")
    }

    fn registry_with_mediator(chain_id: i64, mediator: Address, event: Event) -> AbiRegistry {
        let mut events_by_topic = HashMap::new();
        events_by_topic.insert(event.selector(), event);
        AbiRegistry::from_contracts_for_test(vec![ContractAbi {
            chain_id,
            address: mediator,
            kind: ContractKind::OmnibridgeMediator,
            events_by_topic,
        }])
    }

    fn tokens_bridged_log(
        emitter: Address,
        token: Address,
        recipient: Address,
        message_id: B256,
        value: U256,
        event: &Event,
    ) -> Log {
        let topics = vec![
            event.selector(),
            B256::left_padding_from(token.as_slice()),
            B256::left_padding_from(recipient.as_slice()),
            message_id,
        ];
        let data = Bytes::from(value.to_be_bytes::<32>().to_vec());
        Log {
            inner: alloy::primitives::Log {
                address: emitter,
                data: LogData::new_unchecked(topics, data),
            },
            ..Default::default()
        }
    }

    /// `TokensBridged` is matched by (registered mediator + messageId), even when
    /// the AMB execution `executor` is a different contract than the mediator
    /// that emitted the event (e.g. a Safe recipient forwarding to the mediator).
    /// Regression for completed transfers landing with `token_dst_address` NULL.
    #[test]
    fn find_tokens_bridged_matches_by_message_id_independent_of_executor() {
        let chain_id = 1;
        let mediator = address!("88ad09518695c6c3712ac10a214be5109a655671");
        // Recipient (and AMB executor) is the end-user Safe, NOT the mediator.
        let recipient = address!("9ecf5384f8a2172ec279391b244dcc46cd46e55b");
        let token = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
        let message_id = b256!("00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001dbbe");
        let value = U256::from(22_228_660_000u64);
        let event = tokens_bridged_event();
        let registry = registry_with_mediator(chain_id, mediator, event.clone());

        // The mediator emits `TokensBridged`; the executor differs from it.
        let logs = vec![tokens_bridged_log(
            mediator, token, recipient, message_id, value, &event,
        )];

        let found = find_tokens_bridged(&registry, chain_id, &logs, &message_id)
            .expect("TokensBridged must match by messageId regardless of executor");
        assert_eq!(found.token, token);
        assert_eq!(found.recipient, recipient);
        assert_eq!(found.amount, value);
    }

    #[test]
    fn find_tokens_bridged_ignores_other_message_ids() {
        let chain_id = 1;
        let mediator = address!("88ad09518695c6c3712ac10a214be5109a655671");
        let event = tokens_bridged_event();
        let registry = registry_with_mediator(chain_id, mediator, event.clone());
        let logs = vec![tokens_bridged_log(
            mediator,
            address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            address!("9ecf5384f8a2172ec279391b244dcc46cd46e55b"),
            b256!("00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001dbbe"),
            U256::from(1u64),
            &event,
        )];

        let other_message_id =
            b256!("0000000000000000000000000000000000000000000000000000000000000001");
        assert!(find_tokens_bridged(&registry, chain_id, &logs, &other_message_id).is_none());
    }

    #[test]
    fn is_canonical_message_hash_lookup_requires_current_key_match() {
        let lookup = DashMap::new();
        let message_hash =
            b256!("1111111111111111111111111111111111111111111111111111111111111111");
        let canonical_key = Key::new(10, 1);
        let displaced_key = Key::new(11, 1);

        assert!(
            !is_canonical_message_hash_lookup(&lookup, message_hash, canonical_key),
            "missing lookup must not be treated as canonical",
        );

        lookup.insert(message_hash, canonical_key);
        assert!(is_canonical_message_hash_lookup(
            &lookup,
            message_hash,
            canonical_key,
        ));
        assert!(
            !is_canonical_message_hash_lookup(&lookup, message_hash, displaced_key),
            "a lookup for another key must not drain queued hash events",
        );
    }
}
