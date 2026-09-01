use std::sync::Arc;

use alloy::{
    dyn_abi::{DynSolValue, EventExt},
    primitives::{Address, U256},
    rpc::types::{Block, Log},
};
use anyhow::{Context, Result, bail};

use crate::message_buffer::MessageBuffer;

use super::{
    abi::{AbiRegistry, LogResolution},
    types::{
        AffirmationCompletedEvent, AnnotatedEvent, Direction, Message,
        UserRequestForAffirmationEvent, ValidatorConfirmation, key_from_native_id, native_id_blob,
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
        event: AffirmationCompletedEvent { recipient, value },
        transaction_hash: log.transaction_hash.context("missing tx hash")?,
        block_number: block_number as i64,
        block_timestamp,
    };

    ctx.buffer
        .alter(key, ctx.chain_id as u64, block_number, |message| {
            message.destination_execution = Some(annotated);
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

/// The bridge declares its per-contract nonce as `bytes32` (`bytes32(currentNonce)`
/// in Solidity — a reinterpretation of the same 32 bytes, not a hash), so it
/// decodes as a fixed-bytes value that is read back as a big-endian `U256`.
fn expect_nonce(value: Option<&DynSolValue>, name: &str) -> Result<U256> {
    match value {
        Some(DynSolValue::FixedBytes(value, 32)) => Ok(U256::from_be_slice(value.as_slice())),
        other => bail!("expected bytes32 {name}, got {other:?}"),
    }
}
