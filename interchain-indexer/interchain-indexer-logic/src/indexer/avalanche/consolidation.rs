// SPDX-License-Identifier: LicenseRef-Blockscout

use alloy::{
    hex,
    primitives::{Address, Bytes, ChainId, TxHash},
};
use anyhow::{Context, Result, bail};
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers, sea_orm_active_enums::MessageStatus,
};
use itertools::Itertools;
use sea_orm::{ActiveValue, prelude::BigDecimal};
use std::str::FromStr;

use crate::message_buffer::{Consolidate, ConsolidatedMessage, Key};

use super::{
    abi::{ITokenTransferrer, TeleporterMessage},
    ictt_payload::{CreditExpectation, IcttPayload, PayloadRejection, decode_transferrer_message},
    metrics::AVALANCHE_ICTT_PAYLOAD_OUTCOMES_TOTAL,
    types::{
        AnnotatedEvent, CallOutcome, Message, MessageExecutionOutcome, MessageId, SentOrRouted,
        SentOrRoutedAndCalled, TokenTransfer,
    },
};

/// Data extracted from the source side of a message, unifying the normal
/// (send-event) and fallback (receive/execution-event) paths.
#[derive(Clone, Debug, Default)]
struct SourceData {
    init_timestamp: chrono::NaiveDateTime,
    source_chain_id: ChainId,
    message_id: MessageId,
    source_transaction_hash: Option<TxHash>,
    sender_address: Option<Address>,
    recipient_address: Option<Address>,
    payload: Option<Bytes>,
}

impl SourceData {
    /// Build from the send event (normal path - has all data).
    fn from_send(
        send: &AnnotatedEvent<super::abi::ITeleporterMessenger::SendCrossChainMessage>,
    ) -> Result<Self> {
        Ok(Self {
            init_timestamp: send.block_timestamp,
            source_chain_id: u64::try_from(send.source_chain_id)
                .context("source_chain_id out of range")?,
            message_id: send.event.messageID,
            source_transaction_hash: Some(send.transaction_hash),
            sender_address: Some(send.event.message.originSenderAddress),
            recipient_address: Some(send.event.message.destinationAddress),
            payload: Some(send.event.message.message.clone()),
        })
    }

    /// Build from the receive event (fallback for unknown source chain).
    /// Uses the destination-side timestamp as `init_timestamp`.
    fn from_receive(
        receive: &AnnotatedEvent<super::abi::ITeleporterMessenger::ReceiveCrossChainMessage>,
    ) -> Result<Self> {
        Ok(Self {
            init_timestamp: receive.block_timestamp,
            source_chain_id: u64::try_from(receive.source_chain_id)
                .context("source_chain_id out of range")?,
            message_id: receive.event.messageID,
            ..Default::default()
        })
    }

    /// Build from an execution outcome (fallback for unknown source chain
    /// when only execution events are available).
    fn from_execution(execution: &MessageExecutionOutcome) -> Result<Self> {
        match execution {
            MessageExecutionOutcome::Succeeded(e) => Ok(Self {
                init_timestamp: e.block_timestamp,
                source_chain_id: u64::try_from(e.source_chain_id)
                    .context("source_chain_id out of range")?,
                message_id: e.event.messageID,
                ..Default::default()
            }),
            MessageExecutionOutcome::Failed(e) => Ok(Self {
                init_timestamp: e.block_timestamp,
                source_chain_id: u64::try_from(e.source_chain_id)
                    .context("source_chain_id out of range")?,
                message_id: e.event.messageID,
                ..Default::default()
            }),
        }
    }
}

impl Consolidate for Message {
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>> {
        // Decide if we can consolidate and extract source data.
        //
        // `is_source_unknown_fallback` records whether we took the `(None,
        // true)` arm below (no `send`, source chain unconfigured). It gates
        // incoming-ICTT-transfer reconstruction (Gate B, below): reconstructing
        // while the source chain is configured would race the real `send`
        // event and write a weaker row first.
        let (source_data, is_source_unknown_fallback) =
            match (&self.send, self.source_chain_is_unknown) {
                // Case 1: Have send event - use it (normal path).
                (Some(send), _) => (SourceData::from_send(send)?, false),

                // Case 2: No send, source is UNKNOWN - fall back to receive/execution.
                (None, true) => match (&self.receive, &self.execution) {
                    (Some(receive), _) => (SourceData::from_receive(receive)?, true),
                    (None, Some(exec)) => (SourceData::from_execution(exec)?, true),
                    (None, None) => return Ok(None),
                },

                // Case 3: No send, source is CONFIGURED - wait for send event.
                (None, false) => return Ok(None),
            };

        // Determine status based on execution outcome
        let status = match &self.execution {
            Some(MessageExecutionOutcome::Succeeded(_)) => MessageStatus::Completed,
            Some(MessageExecutionOutcome::Failed(_)) => MessageStatus::Failed,
            None => MessageStatus::Initiated,
        };

        // Collect destination chain IDs from all available events and verify consistency.
        let destination_chain_id = [
            self.send.as_ref().map(|s| s.destination_chain_id),
            self.receive.as_ref().map(|r| r.destination_chain_id),
            self.execution.as_ref().map(|e| match e {
                MessageExecutionOutcome::Succeeded(executed) => executed.destination_chain_id,
                MessageExecutionOutcome::Failed(failed) => failed.destination_chain_id,
            }),
        ]
        .into_iter()
        .flatten()
        .all_equal_value()
        .map_err(|mismatch| {
            anyhow::anyhow!(
                "destination chain id mismatch across events: {mismatch:?} \
                 (send/receive/execution must agree)"
            )
        })?;

        // Get destination-side info from receive/execution events, else fall back to send.
        let (destination_transaction_hash, last_update_timestamp) =
            match (&self.receive, &self.execution) {
                (Some(receive), _) => (
                    receive.transaction_hash.as_slice().to_vec().into(),
                    receive.block_timestamp.into(),
                ),
                (_, Some(MessageExecutionOutcome::Succeeded(executed))) => (
                    executed.transaction_hash.as_slice().to_vec().into(),
                    executed.block_timestamp.into(),
                ),
                (_, Some(MessageExecutionOutcome::Failed(failed))) => (
                    failed.transaction_hash.as_slice().to_vec().into(),
                    failed.block_timestamp.into(),
                ),
                (None, None) => (None, None),
            };

        // Gate A — payload classification. Runs on every consolidate() call
        // that has *any* payload source (send | receive | execution=Failed),
        // regardless of which branch above was taken: it must also cover the
        // fully indexed multi-hop first-leg path, where `send` is present and
        // `is_source_unknown_fallback` is false. Read-only: feeds
        // `is_ictt_complete` and a metric, never builds a row.
        let classified_payload = classify_payload(self);
        record_classification_outcome(key, &classified_payload);

        let credit_expectation = classified_payload
            .as_ref()
            .and_then(|c| c.decoded.as_ref().ok())
            .map(IcttPayload::credit_expectation);

        let is_ictt_complete = ictt_completeness(&self.transfer, credit_expectation);

        let is_execution_succeeded =
            matches!(self.execution, Some(MessageExecutionOutcome::Succeeded(_)));

        // Message is final when:
        // - Execution succeeded (MessageExecuted received), AND
        // - ICTT transfer is complete (if applicable)
        // Failed messages are NOT final - they can be retried via retryMessageExecution()
        let is_final = is_execution_succeeded && is_ictt_complete;

        // Build transfers from ICTT events if present.
        // If transfer building fails (e.g., BigDecimal parsing), propagate the error.
        let transfers = if let Some(send) = self.send.as_ref()
            && let Some(transfer) = self.transfer.as_ref()
        {
            vec![build_transfer(transfer, key, send)?]
        } else if is_source_unknown_fallback {
            // Gate B — reconstruct an incoming ICTT transfer from the ICM
            // payload. Only reachable here because `send` is guaranteed `None`
            // in this branch (see the match above) — never races the
            // `send`-driven path.
            try_reconstruct_transfer(
                self,
                &classified_payload,
                key,
                source_data.source_chain_id,
                &destination_transaction_hash,
            )?
            .into_iter()
            .collect()
        } else {
            Vec::new()
        };

        let message = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            status: ActiveValue::Set(status),
            src_chain_id: ActiveValue::Set(source_data.source_chain_id.try_into()?),
            dst_chain_id: ActiveValue::Set(destination_chain_id.into()),
            native_id: ActiveValue::Set(Some(source_data.message_id.as_slice().to_vec())),
            init_timestamp: ActiveValue::Set(source_data.init_timestamp),
            last_update_timestamp: ActiveValue::Set(last_update_timestamp),
            src_tx_hash: ActiveValue::Set(
                source_data
                    .source_transaction_hash
                    .map(|h| h.as_slice().to_vec()),
            ),
            dst_tx_hash: ActiveValue::Set(destination_transaction_hash),
            sender_address: ActiveValue::Set(
                source_data.sender_address.map(|a| a.as_slice().to_vec()),
            ),
            recipient_address: ActiveValue::Set(
                source_data.recipient_address.map(|a| a.as_slice().to_vec()),
            ),
            payload: ActiveValue::Set(source_data.payload.map(|p| p.to_vec())),
            stats_processed: ActiveValue::Set(0),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
        };

        Ok(Some(ConsolidatedMessage {
            is_final,
            replace_existing: false,
            message,
            transfers,
            amb_confirmations: Vec::new(),
            amb_anomalies: Vec::new(),
        }))
    }
}

fn build_transfer(
    transfer: &TokenTransfer,
    key: &Key,
    send: &AnnotatedEvent<super::abi::ITeleporterMessenger::SendCrossChainMessage>,
) -> Result<crosschain_transfers::ActiveModel> {
    let token_src_chain_id = ActiveValue::Set(send.source_chain_id);
    let token_dst_chain_id = ActiveValue::Set(send.destination_chain_id);

    // token_dst_address: the transferrer this hop actually delivers to.
    // `TeleporterMessage.destinationAddress` is the `ITeleporterReceiver` the
    // ICM message targets, i.e. the transferrer on `send.destination_chain_id`.
    // `SendTokensInput.destinationTokenTransferrerAddress` is the *final*
    // recipient transferrer, which differs from it on a multi-hop first leg —
    // a `TokenRemote` can only address its `TokenHome`, so hop 1's ICM
    // destination is `Home` while `SendTokensInput` names the final chain
    // `R2`. Deriving it from the ICM message keeps `token_dst_chain_id` /
    // `token_dst_address` internally consistent by construction (see
    // task.md `prevent-split-stats-assets`, coding-task-4b.md item 5b).
    let dst_token_addr = send.event.message.destinationAddress;

    match transfer {
        TokenTransfer::Sent(src, dest) => {
            // This should never happen because we cannot call this function without a
            // send event. And if there were sent event, src must be Some.
            let src = src.as_ref().context("missing source side of a transfer")?;
            let (sender, amount, recipient, src_token_addr) = match src {
                SentOrRouted::Sent(e) => (
                    e.event.sender,
                    e.event.amount,
                    e.event.input.recipient,
                    e.contract_address,
                ),
                SentOrRouted::Routed(e) => (
                    alloy::primitives::Address::ZERO, // Routed doesn't have sender
                    e.event.amount,
                    e.event.input.recipient,
                    e.contract_address,
                ),
            };

            let recipient_address = dest
                .as_ref()
                .map(|event| event.recipient)
                .unwrap_or(recipient);
            let model = crosschain_transfers::ActiveModel {
                token_src_chain_id,
                token_dst_chain_id,
                message_id: ActiveValue::Set(key.message_id),
                bridge_id: ActiveValue::Set(key.bridge_id as i32),
                // Always 0 for ICTT transfers
                index: ActiveValue::Set(0),
                sender_address: ActiveValue::Set(sender.as_slice().to_vec().into()),
                src_amount: ActiveValue::Set(Some(BigDecimal::from_str(&amount.to_string())?)),
                dst_amount: ActiveValue::Set(Some(BigDecimal::from_str(&amount.to_string())?)),
                token_src_address: ActiveValue::Set(Some(src_token_addr.as_slice().to_vec())),
                token_dst_address: ActiveValue::Set(Some(dst_token_addr.as_slice().to_vec())),
                recipient_address: ActiveValue::Set(recipient_address.as_slice().to_vec().into()),
                ..Default::default()
            };

            Ok(model)
        }
        TokenTransfer::SentAndCalled(src, dest) => {
            let src = src.as_ref().context("missing source side of a transfer")?;
            // Fill from source event
            let (sender, amount, recipient, fallback, src_token_addr) = match src {
                SentOrRoutedAndCalled::Sent(e) => (
                    e.event.sender,
                    e.event.amount,
                    e.event.input.recipientContract,
                    e.event.input.fallbackRecipient,
                    e.contract_address,
                ),
                SentOrRoutedAndCalled::Routed(e) => (
                    alloy::primitives::Address::ZERO,
                    e.event.amount,
                    e.event.input.recipientContract,
                    e.event.input.fallbackRecipient,
                    e.contract_address,
                ),
            };

            let recipient_address = match dest {
                Some(CallOutcome::Failed(_)) => fallback,
                _ => recipient,
            };

            let model = crosschain_transfers::ActiveModel {
                token_src_chain_id,
                token_dst_chain_id,
                message_id: ActiveValue::Set(key.message_id),
                bridge_id: ActiveValue::Set(key.bridge_id as i32),
                index: ActiveValue::Set(0),
                // Set required chain ID fields
                sender_address: ActiveValue::Set(sender.as_slice().to_vec().into()),
                src_amount: ActiveValue::Set(Some(BigDecimal::from_str(&amount.to_string())?)),
                dst_amount: ActiveValue::Set(Some(BigDecimal::from_str(&amount.to_string())?)),
                token_src_address: ActiveValue::Set(Some(src_token_addr.as_slice().to_vec())),
                token_dst_address: ActiveValue::Set(Some(dst_token_addr.as_slice().to_vec())),
                // If call failed, use fallback recipient
                recipient_address: ActiveValue::Set(recipient_address.as_slice().to_vec().into()),
                ..Default::default()
            };

            Ok(model)
        }
    }
}

/// Which annotated event supplies the ICM payload used for classification
/// (Gate A) and reconstruction (Gate B). Priority: `send` (earliest, and the
/// only source on the fully indexed multi-hop path) → `receive` →
/// `execution = Failed`. `MessageExecuted` carries only `messageID` +
/// `sourceBlockchainID` (`abi.rs`) and is never a payload source.
struct PayloadSource<'a> {
    header: &'a TeleporterMessage,
    source_chain_id: i64,
    destination_chain_id: i64,
}

fn payload_source(msg: &Message) -> Option<PayloadSource<'_>> {
    if let Some(send) = msg.send.as_ref() {
        return Some(PayloadSource {
            header: &send.event.message,
            source_chain_id: send.source_chain_id,
            destination_chain_id: send.destination_chain_id,
        });
    }

    if let Some(receive) = msg.receive.as_ref() {
        return Some(PayloadSource {
            header: &receive.event.message,
            source_chain_id: receive.source_chain_id,
            destination_chain_id: receive.destination_chain_id,
        });
    }

    match &msg.execution {
        Some(MessageExecutionOutcome::Failed(e)) => Some(PayloadSource {
            header: &e.event.message,
            source_chain_id: e.source_chain_id,
            destination_chain_id: e.destination_chain_id,
        }),
        _ => None,
    }
}

/// The ICM payload source plus its decode/classification outcome.
/// `decoded` is `None` = no payload source available yet, never computed.
struct ClassifiedPayload<'a> {
    header: &'a TeleporterMessage,
    source_chain_id: i64,
    destination_chain_id: i64,
    decoded: Result<IcttPayload, PayloadRejection>,
}

fn classify_payload(msg: &Message) -> Option<ClassifiedPayload<'_>> {
    let source = payload_source(msg)?;
    let decoded = decode_transferrer_message(&source.header.message);
    Some(ClassifiedPayload {
        header: source.header,
        source_chain_id: source.source_chain_id,
        destination_chain_id: source.destination_chain_id,
        decoded,
    })
}

/// Gate A metrics/logging. Fires whenever a payload source exists, on *every*
/// `consolidate()` call — including the fully indexed multi-hop first-leg
/// path, where `no_credit_expected` is what makes finality trigger 2
/// observable. Never builds a row; see `try_reconstruct_transfer` for that.
fn record_classification_outcome(key: &Key, classified: &Option<ClassifiedPayload<'_>>) {
    let Some(classified) = classified else {
        return;
    };

    match &classified.decoded {
        Err(_rejection) => {
            record_outcome(key.bridge_id, "rejected_decode");
            tracing::debug!(
                message_id = key.message_id,
                bridge_id = key.bridge_id,
                source_chain_id = classified.source_chain_id,
                reason = "rejected_decode",
                "ICTT payload rejected during classification"
            );
        }
        Ok(payload) if payload.credit_expectation() == CreditExpectation::NotExpected => {
            record_outcome(key.bridge_id, "no_credit_expected");
            tracing::debug!(
                message_id = key.message_id,
                bridge_id = key.bridge_id,
                source_chain_id = classified.source_chain_id,
                reason = "no_credit_expected",
                "ICTT payload never credits this message id (routing intermediate)"
            );
        }
        Ok(_) => {}
    }
}

/// New completeness rule (see `coding-task-1.md` item 5c). `None` credit
/// expectation means "unknown" — no payload source yet, or the payload was
/// rejected — and stays conservative (incomplete), matching today's behavior.
fn ictt_completeness(
    transfer: &Option<TokenTransfer>,
    credit_expectation: Option<CreditExpectation>,
) -> bool {
    let (src_present, dst_present) = match transfer {
        None => return true, // Not an ICTT message.
        Some(TokenTransfer::Sent(src, dst)) => (src.is_some(), dst.is_some()),
        Some(TokenTransfer::SentAndCalled(src, dst)) => (src.is_some(), dst.is_some()),
    };

    matches!(
        (dst_present, src_present, credit_expectation),
        (true, _, _) | (false, true, Some(CreditExpectation::NotExpected))
    )
}

fn record_outcome(bridge_id: i16, outcome: &str) {
    AVALANCHE_ICTT_PAYLOAD_OUTCOMES_TOTAL
        .with_label_values(&[&bridge_id.to_string(), outcome])
        .inc();
}

/// Destination-side ICTT effect observed in the receipt, unified across the
/// two `TokenTransfer` shapes. Read only for `amount` / `CallOutcome` — the
/// payload's `messageType` is authoritative for the recipient rule and
/// `sender_address` (see `try_reconstruct_transfer`'s variant-mismatch
/// handling).
enum DestinationArm<'a> {
    Withdrawn(&'a ITokenTransferrer::TokensWithdrawn),
    Called(&'a CallOutcome),
}

fn destination_arm(transfer: &TokenTransfer) -> Option<DestinationArm<'_>> {
    match transfer {
        TokenTransfer::Sent(_, Some(withdrawn)) => Some(DestinationArm::Withdrawn(withdrawn)),
        TokenTransfer::SentAndCalled(_, Some(outcome)) => Some(DestinationArm::Called(outcome)),
        _ => None,
    }
}

fn destination_arm_amount(arm: &Option<DestinationArm<'_>>) -> Option<alloy::primitives::U256> {
    match arm {
        Some(DestinationArm::Withdrawn(w)) => Some(w.amount),
        Some(DestinationArm::Called(CallOutcome::Succeeded(e))) => Some(e.amount),
        Some(DestinationArm::Called(CallOutcome::Failed(e))) => Some(e.amount),
        None => None,
    }
}

/// Gate B — attempt to build a `crosschain_transfers` row for an incoming
/// ICTT transfer whose source chain is not configured for this bridge, from
/// the ICM payload the destination chain already delivered. Only ever called
/// from `consolidate()`'s `(None, true)` branch (`send` absent, source chain
/// unknown) — never widen this to any other branch, or reconstruction would
/// race the real `send` event and write a weaker row first.
///
/// Every non-reconstructed outcome increments
/// `AVALANCHE_ICTT_PAYLOAD_OUTCOMES_TOTAL` with a distinct label and logs at
/// debug (never above — payload bytes must not be logged at info or higher).
fn try_reconstruct_transfer(
    msg: &Message,
    classified: &Option<ClassifiedPayload<'_>>,
    key: &Key,
    source_chain_id: ChainId,
    dst_tx_hash: &Option<Vec<u8>>,
) -> Result<Option<crosschain_transfers::ActiveModel>> {
    let dst_tx_hash_hex = dst_tx_hash.as_deref().map(hex::encode_prefixed);

    let Some(classified) = classified else {
        record_outcome(key.bridge_id, "skipped_no_payload_source");
        tracing::debug!(
            message_id = key.message_id,
            bridge_id = key.bridge_id,
            source_chain_id,
            dst_tx_hash = ?dst_tx_hash_hex,
            reason = "skipped_no_payload_source",
            "incoming ICTT transfer not reconstructed"
        );
        return Ok(None);
    };

    let payload = match &classified.decoded {
        Ok(payload) => payload,
        Err(_rejection) => {
            // Already counted under `rejected_decode` by Gate A classification;
            // do not double count, just log this specific skip.
            tracing::debug!(
                message_id = key.message_id,
                bridge_id = key.bridge_id,
                source_chain_id,
                dst_tx_hash = ?dst_tx_hash_hex,
                reason = "rejected_decode",
                "incoming ICTT transfer not reconstructed"
            );
            return Ok(None);
        }
    };

    let skip = |reason: &str| {
        record_outcome(key.bridge_id, reason);
        tracing::debug!(
            message_id = key.message_id,
            bridge_id = key.bridge_id,
            source_chain_id,
            dst_tx_hash = ?dst_tx_hash_hex,
            reason,
            "incoming ICTT transfer not reconstructed"
        );
    };

    match payload {
        IcttPayload::RegisterRemote => {
            skip("skipped_register_remote");
            Ok(None)
        }
        IcttPayload::MultiHopSend(_) | IcttPayload::MultiHopCall(_) => {
            skip("skipped_multi_hop");
            Ok(None)
        }
        IcttPayload::SingleHopSend(_) | IcttPayload::SingleHopCall(_) => {
            let Some(transfer) = msg.transfer.as_ref() else {
                skip("skipped_no_destination_event");
                return Ok(None);
            };

            let variant_matches = matches!(
                (payload, transfer),
                (IcttPayload::SingleHopSend(_), TokenTransfer::Sent(_, _))
                    | (
                        IcttPayload::SingleHopCall(_),
                        TokenTransfer::SentAndCalled(_, _)
                    )
            );

            let model = build_reconstructed_transfer(
                classified.header,
                payload,
                transfer,
                key,
                classified.source_chain_id,
                classified.destination_chain_id,
            )?;

            let outcome = if variant_matches {
                "reconstructed"
            } else {
                "variant_mismatch"
            };
            record_outcome(key.bridge_id, outcome);
            tracing::debug!(
                message_id = key.message_id,
                bridge_id = key.bridge_id,
                source_chain_id,
                dst_tx_hash = ?dst_tx_hash_hex,
                outcome,
                "reconstructed incoming ICTT transfer from ICM payload"
            );

            Ok(Some(model))
        }
    }
}

/// Build the reconstructed transfer row for an incoming `SINGLE_HOP_SEND` /
/// `SINGLE_HOP_CALL` message. See the field-mapping table in
/// `coding-task-1.md` item 6. Column-shape uniformity with `build_transfer` is
/// deliberate: both `Set` exactly the same columns (per the "SeaORM
/// `insert_many` Cannot Mix Set and NotSet for the Same Column" gotcha),
/// `sender_address` included — it is always `Set`, `None` for
/// `SINGLE_HOP_SEND`, never `NotSet`.
fn build_reconstructed_transfer(
    header: &TeleporterMessage,
    payload: &IcttPayload,
    transfer: &TokenTransfer,
    key: &Key,
    source_chain_id: i64,
    destination_chain_id: i64,
) -> Result<crosschain_transfers::ActiveModel> {
    let arm = destination_arm(transfer);
    let arm_amount = destination_arm_amount(&arm);

    let (src_amount, dst_amount, recipient_address, sender_address) = match payload {
        IcttPayload::SingleHopSend(send) => (
            send.amount,
            arm_amount.unwrap_or(send.amount),
            send.recipient,
            None,
        ),
        IcttPayload::SingleHopCall(call) => {
            // The payload's messageType is authoritative over the (possibly
            // mismatched) receipt-derived arm: only trust a confirmed
            // `CallOutcome::Failed` for the fallback-recipient rule.
            let recipient = match arm {
                Some(DestinationArm::Called(CallOutcome::Failed(_))) => call.fallbackRecipient,
                _ => call.recipientContract,
            };
            (
                call.amount,
                arm_amount.unwrap_or(call.amount),
                recipient,
                Some(call.originSenderAddress),
            )
        }
        IcttPayload::RegisterRemote
        | IcttPayload::MultiHopSend(_)
        | IcttPayload::MultiHopCall(_) => {
            bail!(
                "build_reconstructed_transfer called with a non-single-hop payload \
                 (message_id={}, bridge_id={})",
                key.message_id,
                key.bridge_id
            );
        }
    };

    Ok(crosschain_transfers::ActiveModel {
        token_src_chain_id: ActiveValue::Set(source_chain_id),
        token_dst_chain_id: ActiveValue::Set(destination_chain_id),
        message_id: ActiveValue::Set(key.message_id),
        bridge_id: ActiveValue::Set(key.bridge_id as i32),
        index: ActiveValue::Set(0),
        sender_address: ActiveValue::Set(sender_address.map(|a: Address| a.as_slice().to_vec())),
        src_amount: ActiveValue::Set(Some(BigDecimal::from_str(&src_amount.to_string())?)),
        dst_amount: ActiveValue::Set(Some(BigDecimal::from_str(&dst_amount.to_string())?)),
        token_src_address: ActiveValue::Set(Some(header.originSenderAddress.as_slice().to_vec())),
        token_dst_address: ActiveValue::Set(Some(header.destinationAddress.as_slice().to_vec())),
        recipient_address: ActiveValue::Set(Some(recipient_address.as_slice().to_vec())),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use alloy::{
        primitives::{B256, U256},
        sol_types::SolValue,
    };

    use super::*;
    use crate::indexer::avalanche::abi::{
        ITeleporterMessenger, ITokenTransferrer, MultiHopCallMessage, MultiHopSendMessage,
        RegisterRemoteMessage, SendTokensInput, SingleHopCallMessage, SingleHopSendMessage,
        TeleporterFeeInfo, TeleporterMessage, TransferrerMessage,
    };

    fn addr(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    fn key() -> Key {
        Key::new(1, 1)
    }

    /// Builds a `send` event whose ICM destination address (`message.destinationAddress`)
    /// and ICTT `SendTokensInput.destinationTokenTransferrerAddress` can be set
    /// independently, so tests can construct both single-hop (equal) and
    /// multi-hop first-leg (different) scenarios.
    fn send_event(
        icm_destination_address: Address,
    ) -> AnnotatedEvent<ITeleporterMessenger::SendCrossChainMessage> {
        AnnotatedEvent {
            event: ITeleporterMessenger::SendCrossChainMessage {
                messageID: B256::from([0x01u8; 32]),
                destinationBlockchainID: B256::from([0x02u8; 32]),
                message: TeleporterMessage {
                    messageNonce: U256::from(1u64),
                    originSenderAddress: addr(0x01),
                    destinationBlockchainID: B256::from([0x02u8; 32]),
                    destinationAddress: icm_destination_address,
                    requiredGasLimit: U256::from(100_000u64),
                    allowedRelayerAddresses: vec![],
                    receipts: vec![],
                    message: Bytes::new(),
                },
                feeInfo: TeleporterFeeInfo {
                    feeTokenAddress: Address::ZERO,
                    amount: U256::ZERO,
                },
            },
            transaction_hash: B256::from([0x03u8; 32]),
            block_number: 100,
            block_timestamp: chrono::Utc::now().naive_utc(),
            source_chain_id: 1,
            destination_chain_id: 100,
        }
    }

    fn tokens_sent_transfer(
        destination_token_transferrer_address: Address,
        sender: Address,
        recipient: Address,
        src_token_contract: Address,
        amount: u64,
    ) -> TokenTransfer {
        TokenTransfer::Sent(
            Some(SentOrRouted::Sent(
                super::super::types::AnnotatedICTTSource {
                    event: ITokenTransferrer::TokensSent {
                        teleporterMessageID: B256::from([0x01u8; 32]),
                        sender,
                        input: SendTokensInput {
                            destinationBlockchainID: B256::from([0x02u8; 32]),
                            destinationTokenTransferrerAddress:
                                destination_token_transferrer_address,
                            recipient,
                            primaryFeeTokenAddress: Address::ZERO,
                            primaryFee: U256::ZERO,
                            secondaryFee: U256::ZERO,
                            requiredGasLimit: U256::from(100_000u64),
                            multiHopFallback: Address::ZERO,
                        },
                        amount: U256::from(amount),
                    },
                    contract_address: src_token_contract,
                },
            )),
            None,
        )
    }

    fn dst_token_address(model: &crosschain_transfers::ActiveModel) -> Option<Vec<u8>> {
        match &model.token_dst_address {
            ActiveValue::Set(v) | ActiveValue::Unchanged(v) => v.clone(),
            ActiveValue::NotSet => panic!("token_dst_address must be set"),
        }
    }

    /// Single-hop send: the ICM destination and the ICTT transferrer agree
    /// (the normal case for 100% of today's traffic). The fix must be a
    /// provable no-op here.
    #[test]
    fn test_build_transfer_single_hop_dst_address_unchanged() {
        let icm_and_ictt_transferrer = addr(0xaa);
        let send = send_event(icm_and_ictt_transferrer);
        let transfer = tokens_sent_transfer(
            icm_and_ictt_transferrer,
            addr(0x11),
            addr(0x22),
            addr(0x33),
            1_000,
        );

        let model = build_transfer(&transfer, &key(), &send).unwrap();

        assert_eq!(
            dst_token_address(&model),
            Some(icm_and_ictt_transferrer.as_slice().to_vec())
        );
    }

    /// Synthetic multi-hop first leg: `message.destinationAddress` (Home's
    /// ICM receiver) differs from `SendTokensInput.destinationTokenTransferrerAddress`
    /// (the final R2 transferrer). `token_dst_address` must follow the ICM
    /// message (i.e. `token_dst_chain_id`'s own chain), not the ICTT input.
    #[test]
    fn test_build_transfer_multi_hop_dst_address_follows_icm_message() {
        let icm_destination_on_home = addr(0xaa);
        let final_hop_transferrer_on_r2 = addr(0xbb);
        assert_ne!(icm_destination_on_home, final_hop_transferrer_on_r2);

        let send = send_event(icm_destination_on_home);
        let transfer = tokens_sent_transfer(
            final_hop_transferrer_on_r2,
            addr(0x11),
            addr(0x22),
            addr(0x33),
            1_000,
        );

        let model = build_transfer(&transfer, &key(), &send).unwrap();

        assert_eq!(
            dst_token_address(&model),
            Some(icm_destination_on_home.as_slice().to_vec()),
            "token_dst_address must follow token_dst_chain_id's chain (the ICM hop \
             destination), not the ICTT input's final transferrer"
        );
    }

    // --- Incoming ICTT reconstruction (Gate A + Gate B) ---

    fn set_value<T: Clone + Into<sea_orm::Value>>(av: &ActiveValue<T>) -> T {
        match av {
            ActiveValue::Set(v) | ActiveValue::Unchanged(v) => v.clone(),
            ActiveValue::NotSet => panic!("expected ActiveValue::Set"),
        }
    }

    fn message_id() -> B256 {
        B256::from([0x01u8; 32])
    }

    /// Like `send_event`, but with a caller-supplied ICM payload so the
    /// (send-present) classification path can be exercised too.
    fn send_event_with_payload(
        icm_destination_address: Address,
        message_bytes: Bytes,
    ) -> AnnotatedEvent<ITeleporterMessenger::SendCrossChainMessage> {
        let mut event = send_event(icm_destination_address);
        event.event.message.message = message_bytes;
        event
    }

    fn receive_event(
        message_bytes: Bytes,
        origin_sender_address: Address,
        destination_address: Address,
        source_chain_id: i64,
        destination_chain_id: i64,
    ) -> AnnotatedEvent<ITeleporterMessenger::ReceiveCrossChainMessage> {
        AnnotatedEvent {
            event: ITeleporterMessenger::ReceiveCrossChainMessage {
                messageID: message_id(),
                sourceBlockchainID: B256::from([0x02u8; 32]),
                deliverer: Address::ZERO,
                rewardRedeemer: Address::ZERO,
                message: TeleporterMessage {
                    messageNonce: U256::from(1u64),
                    originSenderAddress: origin_sender_address,
                    destinationBlockchainID: B256::from([0x02u8; 32]),
                    destinationAddress: destination_address,
                    requiredGasLimit: U256::from(100_000u64),
                    allowedRelayerAddresses: vec![],
                    receipts: vec![],
                    message: message_bytes,
                },
            },
            transaction_hash: B256::from([0x05u8; 32]),
            block_number: 200,
            block_timestamp: chrono::Utc::now().naive_utc(),
            source_chain_id,
            destination_chain_id,
        }
    }

    fn execution_succeeded(
        source_chain_id: i64,
        destination_chain_id: i64,
    ) -> MessageExecutionOutcome {
        MessageExecutionOutcome::Succeeded(AnnotatedEvent {
            event: ITeleporterMessenger::MessageExecuted {
                messageID: message_id(),
                sourceBlockchainID: B256::from([0x02u8; 32]),
            },
            transaction_hash: B256::from([0x06u8; 32]),
            block_number: 200,
            block_timestamp: chrono::Utc::now().naive_utc(),
            source_chain_id,
            destination_chain_id,
        })
    }

    fn encode_transferrer(message_type: u8, inner: Vec<u8>) -> Bytes {
        TransferrerMessage {
            messageType: message_type,
            payload: inner.into(),
        }
        .abi_encode()
        .into()
    }

    fn single_hop_send_payload(recipient: Address, amount: u64) -> Bytes {
        let inner = SingleHopSendMessage {
            recipient,
            amount: U256::from(amount),
        }
        .abi_encode();
        encode_transferrer(1, inner)
    }

    fn single_hop_call_payload(
        origin_sender_address: Address,
        recipient_contract: Address,
        fallback_recipient: Address,
        amount: u64,
    ) -> Bytes {
        let inner = SingleHopCallMessage {
            sourceBlockchainID: B256::ZERO,
            originTokenTransferrerAddress: Address::ZERO,
            originSenderAddress: origin_sender_address,
            recipientContract: recipient_contract,
            amount: U256::from(amount),
            recipientPayload: Bytes::new(),
            recipientGasLimit: U256::ZERO,
            fallbackRecipient: fallback_recipient,
        }
        .abi_encode();
        encode_transferrer(2, inner)
    }

    fn register_remote_payload() -> Bytes {
        let inner = RegisterRemoteMessage {
            initialReserveImbalance: U256::ZERO,
            homeTokenDecimals: 18,
            remoteTokenDecimals: 18,
        }
        .abi_encode();
        encode_transferrer(0, inner)
    }

    fn multi_hop_send_payload(recipient: Address, amount: u64) -> Bytes {
        let inner = MultiHopSendMessage {
            destinationBlockchainID: B256::ZERO,
            destinationTokenTransferrerAddress: Address::ZERO,
            recipient,
            amount: U256::from(amount),
            secondaryFee: U256::ZERO,
            secondaryGasLimit: U256::ZERO,
            multiHopFallback: Address::ZERO,
        }
        .abi_encode();
        encode_transferrer(3, inner)
    }

    fn multi_hop_call_payload(amount: u64) -> Bytes {
        let inner = MultiHopCallMessage {
            originSenderAddress: Address::ZERO,
            destinationBlockchainID: B256::ZERO,
            destinationTokenTransferrerAddress: Address::ZERO,
            recipientContract: Address::ZERO,
            amount: U256::from(amount),
            recipientPayload: Bytes::new(),
            recipientGasLimit: U256::ZERO,
            fallbackRecipient: Address::ZERO,
            secondaryRequiredGasLimit: U256::ZERO,
            multiHopFallback: Address::ZERO,
            secondaryFee: U256::ZERO,
        }
        .abi_encode();
        encode_transferrer(4, inner)
    }

    fn withdrawn_transfer(amount: u64) -> TokenTransfer {
        TokenTransfer::Sent(
            None,
            Some(ITokenTransferrer::TokensWithdrawn {
                recipient: addr(0x77),
                amount: U256::from(amount),
            }),
        )
    }

    fn call_outcome_transfer(outcome: CallOutcome) -> TokenTransfer {
        TokenTransfer::SentAndCalled(None, Some(outcome))
    }

    /// Happy path: `X -> A`, `X` unconfigured, `SINGLE_HOP_SEND` payload plus
    /// a corroborating `TokensWithdrawn`. This is the case the whole task
    /// exists to close.
    #[test]
    fn test_consolidate_reconstructs_incoming_single_hop_send_transfer() {
        let origin_sender = addr(0x33);
        let destination_address = addr(0x01);
        let payload_recipient = addr(0x71);
        let message_bytes = single_hop_send_payload(payload_recipient, 21_633);

        let message = Message {
            receive: Some(receive_event(
                message_bytes,
                origin_sender,
                destination_address,
                43114,
                8021,
            )),
            execution: Some(execution_succeeded(43114, 8021)),
            transfer: Some(withdrawn_transfer(21_633)),
            source_chain_is_unknown: true,
            ..Default::default()
        };

        let consolidated = message
            .consolidate(&key())
            .unwrap()
            .expect("must consolidate");

        assert!(
            consolidated.is_final,
            "credit observed via TokensWithdrawn, message must be final"
        );
        assert_eq!(consolidated.transfers.len(), 1);
        let t = &consolidated.transfers[0];
        assert_eq!(set_value(&t.token_src_chain_id), 43114);
        assert_eq!(set_value(&t.token_dst_chain_id), 8021);
        assert_eq!(
            set_value(&t.token_src_address),
            Some(origin_sender.as_slice().to_vec()),
            "token_src_address must be byte-identical to what the outgoing path writes"
        );
        assert_eq!(
            set_value(&t.token_dst_address),
            Some(destination_address.as_slice().to_vec())
        );
        assert_eq!(
            set_value(&t.recipient_address),
            Some(payload_recipient.as_slice().to_vec())
        );
        assert_eq!(set_value(&t.sender_address), None);
        assert_eq!(set_value(&t.src_amount), Some(BigDecimal::from(21_633u64)));
        assert_eq!(set_value(&t.dst_amount), Some(BigDecimal::from(21_633u64)));

        // Reconstruction must never populate source-side message columns.
        let m = &consolidated.message;
        assert_eq!(set_value(&m.src_tx_hash), None);
        assert_eq!(set_value(&m.sender_address), None);
        assert_eq!(set_value(&m.payload), None);
    }

    #[test]
    fn test_consolidate_single_hop_call_succeeded_uses_recipient_contract() {
        let origin_sender = addr(0x33);
        let destination_address = addr(0x01);
        let recipient_contract = addr(0x44);
        let fallback_recipient = addr(0x55);
        let message_bytes =
            single_hop_call_payload(origin_sender, recipient_contract, fallback_recipient, 500);

        let message = Message {
            receive: Some(receive_event(
                message_bytes,
                origin_sender,
                destination_address,
                43114,
                8021,
            )),
            execution: Some(execution_succeeded(43114, 8021)),
            transfer: Some(call_outcome_transfer(CallOutcome::Succeeded(
                ITokenTransferrer::CallSucceeded {
                    recipientContract: recipient_contract,
                    amount: U256::from(500u64),
                },
            ))),
            source_chain_is_unknown: true,
            ..Default::default()
        };

        let consolidated = message
            .consolidate(&key())
            .unwrap()
            .expect("must consolidate");
        let t = &consolidated.transfers[0];

        assert_eq!(
            set_value(&t.recipient_address),
            Some(recipient_contract.as_slice().to_vec())
        );
        assert_eq!(
            set_value(&t.sender_address),
            Some(origin_sender.as_slice().to_vec()),
            "SINGLE_HOP_CALL must use payload.originSenderAddress as sender_address"
        );
    }

    #[test]
    fn test_consolidate_single_hop_call_failed_uses_fallback_recipient() {
        let origin_sender = addr(0x33);
        let destination_address = addr(0x01);
        let recipient_contract = addr(0x44);
        let fallback_recipient = addr(0x55);
        let message_bytes =
            single_hop_call_payload(origin_sender, recipient_contract, fallback_recipient, 500);

        let message = Message {
            receive: Some(receive_event(
                message_bytes,
                origin_sender,
                destination_address,
                43114,
                8021,
            )),
            execution: Some(execution_succeeded(43114, 8021)),
            transfer: Some(call_outcome_transfer(CallOutcome::Failed(
                ITokenTransferrer::CallFailed {
                    recipientContract: recipient_contract,
                    amount: U256::from(500u64),
                },
            ))),
            source_chain_is_unknown: true,
            ..Default::default()
        };

        let consolidated = message
            .consolidate(&key())
            .unwrap()
            .expect("must consolidate");
        let t = &consolidated.transfers[0];

        assert_eq!(
            set_value(&t.recipient_address),
            Some(fallback_recipient.as_slice().to_vec())
        );
        assert_eq!(
            set_value(&t.sender_address),
            Some(origin_sender.as_slice().to_vec())
        );
    }

    #[test]
    fn test_consolidate_register_remote_produces_no_transfer() {
        let message = Message {
            receive: Some(receive_event(
                register_remote_payload(),
                addr(0x33),
                addr(0x01),
                43114,
                8021,
            )),
            execution: Some(execution_succeeded(43114, 8021)),
            transfer: None,
            source_chain_is_unknown: true,
            ..Default::default()
        };

        let consolidated = message
            .consolidate(&key())
            .unwrap()
            .expect("must consolidate (messaging-only, no ICTT transfer)");

        assert!(consolidated.transfers.is_empty());
    }

    /// Trigger-2 regression: a fully indexed multi-hop first leg (`send`
    /// present, source chain configured) whose home routes onward instead of
    /// crediting a recipient. Before this task this message was `Partial`
    /// forever; classification must now recognize `MULTI_HOP_SEND` as "no
    /// credit expected" and let it finalize on the `send`-driven row alone.
    #[test]
    fn test_consolidate_multi_hop_first_leg_with_no_destination_credit_becomes_final() {
        let icm_destination = addr(0xaa);
        let send =
            send_event_with_payload(icm_destination, multi_hop_send_payload(addr(0x22), 1_000));
        let transfer = tokens_sent_transfer(addr(0xbb), addr(0x11), addr(0x22), addr(0x33), 1_000);

        let message = Message {
            send: Some(send.clone()),
            execution: Some(execution_succeeded(
                send.source_chain_id,
                send.destination_chain_id,
            )),
            transfer: Some(transfer),
            source_chain_is_unknown: false,
            ..Default::default()
        };

        let consolidated = message
            .consolidate(&key())
            .unwrap()
            .expect("must consolidate");

        assert!(
            consolidated.is_final,
            "a multi-hop first leg with src present and no destination credit \
             must become final once classified as a routing intermediate"
        );
        assert_eq!(
            consolidated.transfers.len(),
            1,
            "the send-driven row is still built as today"
        );
    }

    /// Same classification, but via `MULTI_HOP_CALL` and taken from the
    /// `(None, true)` fallback path — must still skip row reconstruction.
    #[test]
    fn test_consolidate_multi_hop_call_produces_no_reconstructed_row() {
        let message = Message {
            receive: Some(receive_event(
                multi_hop_call_payload(1_000),
                addr(0x33),
                addr(0x01),
                43114,
                8021,
            )),
            execution: Some(execution_succeeded(43114, 8021)),
            transfer: Some(withdrawn_transfer(1_000)),
            source_chain_is_unknown: true,
            ..Default::default()
        };

        let consolidated = message
            .consolidate(&key())
            .unwrap()
            .expect("must consolidate");

        assert!(
            consolidated.transfers.is_empty(),
            "a multi-hop routing intermediate must never produce an \
             `R1 -> home` transfer row"
        );
    }

    #[test]
    fn test_consolidate_configured_source_without_send_returns_none_even_with_decodable_payload() {
        let message = Message {
            receive: Some(receive_event(
                single_hop_send_payload(addr(0x71), 1_000),
                addr(0x33),
                addr(0x01),
                43114,
                8021,
            )),
            execution: Some(execution_succeeded(43114, 8021)),
            transfer: Some(withdrawn_transfer(1_000)),
            source_chain_is_unknown: false,
            ..Default::default()
        };

        let consolidated = message.consolidate(&key()).unwrap();

        assert!(
            consolidated.is_none(),
            "a configured-source message must wait for `send`, never reconstruct \
             from the payload"
        );
    }

    #[test]
    fn test_consolidate_no_destination_event_produces_no_reconstructed_row() {
        let message = Message {
            receive: Some(receive_event(
                single_hop_send_payload(addr(0x71), 1_000),
                addr(0x33),
                addr(0x01),
                43114,
                8021,
            )),
            execution: Some(execution_succeeded(43114, 8021)),
            transfer: None,
            source_chain_is_unknown: true,
            ..Default::default()
        };

        let consolidated = message
            .consolidate(&key())
            .unwrap()
            .expect("must consolidate");

        assert!(
            consolidated.transfers.is_empty(),
            "a decodable SINGLE_HOP_SEND with no corroborating receiver-side \
             ICTT effect must not be reconstructed"
        );
    }

    /// The payload's `messageType` is authoritative over the receipt-derived
    /// `TokenTransfer` variant: here the payload says `SINGLE_HOP_CALL` but
    /// the observed arm is the plain `Sent` shape (as if log classification
    /// picked the wrong variant). The row must still be built from the
    /// payload rule.
    #[test]
    fn test_consolidate_variant_mismatch_builds_row_from_payload_rule() {
        let origin_sender = addr(0x33);
        let destination_address = addr(0x01);
        let recipient_contract = addr(0x44);
        let fallback_recipient = addr(0x55);
        let message_bytes =
            single_hop_call_payload(origin_sender, recipient_contract, fallback_recipient, 500);

        let message = Message {
            receive: Some(receive_event(
                message_bytes,
                origin_sender,
                destination_address,
                43114,
                8021,
            )),
            execution: Some(execution_succeeded(43114, 8021)),
            transfer: Some(withdrawn_transfer(500)),
            source_chain_is_unknown: true,
            ..Default::default()
        };

        let consolidated = message
            .consolidate(&key())
            .unwrap()
            .expect("must consolidate");

        assert_eq!(consolidated.transfers.len(), 1);
        let t = &consolidated.transfers[0];
        assert_eq!(
            set_value(&t.recipient_address),
            Some(recipient_contract.as_slice().to_vec()),
            "no confirmed CallOutcome::Failed, so the non-fallback recipient rule applies"
        );
        assert_eq!(
            set_value(&t.sender_address),
            Some(origin_sender.as_slice().to_vec()),
            "sender_address must still follow the payload variant (SINGLE_HOP_CALL), \
             not the mismatched Sent arm"
        );
        assert_eq!(set_value(&t.dst_amount), Some(BigDecimal::from(500u64)));
    }

    #[test]
    fn test_consolidate_non_ictt_payload_with_destination_event_produces_no_transfer() {
        // Arbitrary bytes that do not decode as a `TransferrerMessage` at all.
        let message = Message {
            receive: Some(receive_event(
                Bytes::from_static(b"not an ICTT payload"),
                addr(0x33),
                addr(0x01),
                43114,
                8021,
            )),
            execution: Some(execution_succeeded(43114, 8021)),
            transfer: Some(withdrawn_transfer(1_000)),
            source_chain_is_unknown: true,
            ..Default::default()
        };

        let consolidated = message
            .consolidate(&key())
            .unwrap()
            .expect("must consolidate");

        assert!(
            consolidated.transfers.is_empty(),
            "misdecoding an arbitrary payload into a bogus transfer is a \
             correctness failure, not a cosmetic one"
        );
    }
}
