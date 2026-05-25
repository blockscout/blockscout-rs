use std::{collections::HashMap, sync::Arc};

use alloy::{
    dyn_abi::{DynSolValue, EventExt},
    primitives::{Address, B256, keccak256},
    rpc::types::{Block, Log},
};
use anyhow::{Context, Result, bail};
use dashmap::DashMap;

use crate::message_buffer::{Key, MessageBuffer};

use super::{
    abi::{AbiRegistry, ContractKind},
    header::parse_amb_header,
    payload_processor::{PayloadDecodeContext, PayloadProcessor},
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
    pub(super) payload_processors: &'a [Box<dyn PayloadProcessor>],
    pub(super) buffer: &'a Arc<MessageBuffer<Message>>,
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

pub(super) async fn dispatch_transaction(
    ctx: &EventContext<'_>,
    receipt_logs: &[Log],
    block: &Block,
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

        let result = match event.name.as_str() {
            "UserRequestForAffirmation" => {
                handle_source_request(
                    ctx,
                    event,
                    log,
                    receipt_logs,
                    block_timestamp,
                    Direction::ForeignToHome,
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
    let source_chain_id = header.source_chain_id;
    let block_number = log.block_number.context("missing block number")?;
    let destination_chain_id = header.destination_chain_id;
    let application_calldata = encoded_data[header.payload_offset..].to_vec();
    let key = key_from_message_id(&message_id, ctx.bridge_id)?;
    let message_hash = keccak256(&encoded_data);
    ctx.message_hash_lookup.insert(message_hash, key);

    let annotated = AnnotatedEvent {
        event: SourceRequestEvent {
            message_id,
            encoded_data,
            application_calldata,
            header: header.into(),
        },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
        source_chain_id,
        destination_chain_id,
    };

    ctx.buffer
        .alter(
            key,
            ctx.chain_id as u64,
            annotated.block_number as u64,
            |message| {
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
        ctx.buffer
            .alter(key, ctx.chain_id as u64, block_number, |message| {
                message.source_transfer = Some(source_transfer);
                Ok(())
            })
            .await?;
    }

    drain_pending_message_hash_events(ctx, message_hash, key).await?;
    maybe_decode_payload(ctx, key).await
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
/// and amount so payload decoding can be retried after source-side data arrives.
fn find_tokens_bridged(
    ctx: &EventContext<'_>,
    receipt_logs: &[Log],
    executor: Address,
    message_id: &B256,
) -> Option<DestinationTransferDetails> {
    for log in receipt_logs {
        if log.address() != executor {
            continue;
        }
        let Some(topic) = log.topic0() else {
            continue;
        };
        let Some((event, kind)) =
            ctx.abi_registry
                .event_for_log(ctx.chain_id, log.address(), topic)
        else {
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
    let destination_transfer = find_tokens_bridged(ctx, receipt_logs, executor, &message_id);

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            message.destination_execution = Some(match kind {
                DestinationKind::Affirmation => DestinationExecution::Affirmation(annotated),
                DestinationKind::Relayed => DestinationExecution::Relayed(annotated),
            });
            if let Some(destination_transfer) = destination_transfer {
                message.destination_transfer = Some(destination_transfer);
            }
            Ok(())
        })
        .await?;

    maybe_decode_payload(ctx, key).await?;

    Ok(Some(key))
}

async fn maybe_decode_payload(ctx: &EventContext<'_>, key: Key) -> Result<()> {
    let (source_request, destination_execution, destination_transfer, has_decoded_payload) = {
        let entry = ctx.buffer.get_mut_or_default(key).await?;
        (
            entry.inner.source_request.clone(),
            entry.inner.destination_execution.clone(),
            entry.inner.destination_transfer.clone(),
            entry.inner.decoded_payload.is_some(),
        )
    };
    if has_decoded_payload && destination_transfer.is_none() {
        return Ok(());
    }
    let Some(source_request) = source_request else {
        return Ok(());
    };
    let source_event = source_request.event();
    let (destination_chain_id, executor, message_id, mutation_chain_id, mutation_block_number) =
        match destination_execution.as_ref() {
            Some(destination_execution) => {
                let destination_event = destination_execution.event();
                (
                    destination_event.destination_chain_id,
                    destination_event.event.executor,
                    destination_event.event.message_id,
                    destination_event.destination_chain_id as u64,
                    destination_event.block_number as u64,
                )
            }
            None => (
                source_event.event.header.destination_chain_id,
                source_event.event.header.executor,
                source_event.event.message_id,
                source_event.source_chain_id as u64,
                source_event.block_number as u64,
            ),
        };

    for processor in ctx.payload_processors {
        if !processor.matches(destination_chain_id, executor) {
            continue;
        }
        let decode_ctx = PayloadDecodeContext {
            dst_chain_id: destination_chain_id,
            executor,
            sender: source_event.event.header.sender,
            message_id,
            application_calldata: &source_event.event.application_calldata,
            destination_transfer: destination_transfer.as_ref(),
            abi_registry: ctx.abi_registry,
        };
        if let Some(decoded_payload) = processor.decode(&decode_ctx)? {
            ctx.buffer
                .alter(key, mutation_chain_id, mutation_block_number, |message| {
                    message.decoded_payload = Some(decoded_payload);
                    Ok(())
                })
                .await?;
            break;
        }
    }

    Ok(())
}

fn key_from_message_id(message_id: &B256, bridge_id: i32) -> Result<Key> {
    let bytes: [u8; 8] = message_id.as_slice()[24..32].try_into()?;
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
