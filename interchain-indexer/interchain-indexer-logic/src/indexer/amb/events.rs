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
        Direction, Message, SourceRequest, SourceRequestEvent, SourceTransferDetails,
        ValidatorConfirmation,
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
    validator_confirmations: HashMap<Address, ValidatorConfirmation>,
    signatures_collected: Option<AnnotatedEvent<CollectedSignaturesEvent>>,
}

pub(super) async fn dispatch_transaction(
    ctx: &EventContext<'_>,
    receipt_logs: &[Log],
    block: &Block,
) -> Result<()> {
    let block_timestamp = chrono::DateTime::from_timestamp(block.header.timestamp as i64, 0)
        .map(|dt| dt.naive_utc())
        .context("invalid block timestamp")?;

    let mut destination_for_payload: Option<(Key, DestinationExecutionEvent)> = None;

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
                    Direction::EthToGnosis,
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
                    Direction::GnosisToEth,
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
                    block_timestamp,
                    DestinationKind::Affirmation,
                )
                .await;
                if let Ok(Some(value)) = &out {
                    destination_for_payload = Some(value.clone());
                }
                out.map(|_| ())
            }
            "RelayedMessage" => {
                let out = handle_destination_execution(
                    ctx,
                    event,
                    log,
                    block_timestamp,
                    DestinationKind::Relayed,
                )
                .await;
                if let Ok(Some(value)) = &out {
                    destination_for_payload = Some(value.clone());
                }
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

    if let Some((key, destination)) = destination_for_payload {
        maybe_decode_payload(ctx, receipt_logs, key, destination).await?;
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
        (Direction::EthToGnosis, AmbSide::Foreign) | (Direction::GnosisToEth, AmbSide::Home) => {}
        _ => bail!("source request emitted on unexpected AMB side"),
    }

    let header = parse_amb_header(&encoded_data, header_layout)?;
    let source_chain_id = header.source_chain_id;
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
        block_number: log.block_number.context("missing block number")? as i64,
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
                    Direction::EthToGnosis => SourceRequest::Affirmation(annotated),
                    Direction::GnosisToEth => SourceRequest::Signature(annotated),
                });
                Ok(())
            },
        )
        .await?;

    if let Some(source_transfer) =
        find_tokens_bridging_initiated(ctx, receipt_logs, &message_id)
    {
        ctx.buffer
            .alter(key, ctx.chain_id as u64, 0, |message| {
                message.source_transfer = Some(source_transfer);
                Ok(())
            })
            .await?;
    }

    drain_pending_message_hash_events(ctx, message_hash, key).await
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
        let topic = log.topic0()?;
        let (event, kind) =
            ctx.abi_registry
                .event_for_log(ctx.chain_id, log.address(), topic)?;
        if !matches!(kind, ContractKind::OmnibridgeMediator)
            || event.name != "TokensBridgingInitiated"
        {
            continue;
        }
        let decoded = event.decode_log(log.data()).ok()?;
        let token = match decoded.indexed.first()? {
            DynSolValue::Address(value) => *value,
            _ => continue,
        };
        let sender = match decoded.indexed.get(1)? {
            DynSolValue::Address(value) => *value,
            _ => continue,
        };
        let event_message_id = match decoded.indexed.get(2)? {
            DynSolValue::FixedBytes(value, 32) => *value,
            _ => continue,
        };
        if &event_message_id != message_id {
            continue;
        }
        let amount = match decoded.body.first()? {
            DynSolValue::Uint(value, _) => *value,
            _ => continue,
        };
        return Some(SourceTransferDetails {
            token,
            sender,
            amount,
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
        Some(key) => apply_validator_confirmation(ctx, key, confirmation).await,
        None => {
            ctx.pending_message_hash_events
                .entry(message_hash)
                .or_default()
                .validator_confirmations
                .insert(signer, confirmation);
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
    let annotated = AnnotatedEvent {
        event: CollectedSignaturesEvent {
            authority_responsible_for_relay: authority,
            message_hash,
            count,
        },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
        source_chain_id: 100,
        destination_chain_id: 1,
    };

    match ctx.message_hash_lookup.get(&message_hash).map(|key| *key) {
        Some(key) => apply_collected_signatures(ctx, key, annotated).await,
        None => {
            ctx.pending_message_hash_events
                .entry(message_hash)
                .or_default()
                .signatures_collected = Some(annotated);
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

    for confirmation in pending.validator_confirmations.into_values() {
        apply_validator_confirmation(ctx, key, confirmation).await?;
    }

    if let Some(signatures_collected) = pending.signatures_collected {
        apply_collected_signatures(ctx, key, signatures_collected).await?;
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
    confirmation: ValidatorConfirmation,
) -> Result<()> {
    let block_number = confirmation.block_number;
    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
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
    annotated: AnnotatedEvent<CollectedSignaturesEvent>,
) -> Result<()> {
    let block_number = u64::try_from(annotated.block_number)
        .context("collected-signatures block number out of range")?;
    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
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
    block_timestamp: chrono::NaiveDateTime,
    kind: DestinationKind,
) -> Result<Option<(Key, DestinationExecutionEvent)>> {
    let decoded = event.decode_log(log.data())?;
    let sender = expect_address(decoded.indexed.first(), "sender")?;
    let executor = expect_address(decoded.indexed.get(1), "executor")?;
    let message_id = expect_b256(decoded.indexed.get(2), "messageId")?;
    let status = expect_bool(decoded.body.first(), "status")?;
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
        source_chain_id: if matches!(kind, DestinationKind::Affirmation) {
            1
        } else {
            100
        },
        destination_chain_id: ctx.chain_id,
    };

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            message.destination_execution = Some(match kind {
                DestinationKind::Affirmation => DestinationExecution::Affirmation(annotated),
                DestinationKind::Relayed => DestinationExecution::Relayed(annotated),
            });
            Ok(())
        })
        .await?;

    Ok(Some((key, execution)))
}

async fn maybe_decode_payload(
    ctx: &EventContext<'_>,
    receipt_logs: &[Log],
    key: Key,
    destination: DestinationExecutionEvent,
) -> Result<()> {
    let mut source_request = None;
    ctx.buffer
        .alter(key, ctx.chain_id as u64, 0, |message| {
            source_request = message.source_request.clone();
            Ok(())
        })
        .await?;
    let Some(source_request) = source_request else {
        return Ok(());
    };
    let source_event = source_request.event();

    for processor in ctx.payload_processors {
        if !processor.matches(ctx.chain_id, destination.executor) {
            continue;
        }
        let decode_ctx = PayloadDecodeContext {
            dst_chain_id: ctx.chain_id,
            executor: destination.executor,
            sender: source_event.event.header.sender,
            message_id: destination.message_id,
            application_calldata: &source_event.event.application_calldata,
            destination_receipt_logs: receipt_logs,
            abi_registry: ctx.abi_registry,
        };
        if let Some(decoded_payload) = processor.decode(&decode_ctx)? {
            ctx.buffer
                .alter(key, ctx.chain_id as u64, 0, |message| {
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
