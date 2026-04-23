use alloy::primitives::{Address, Bytes, ChainId, TxHash};
use anyhow::{Context, Result};
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers, sea_orm_active_enums::MessageStatus,
};
use itertools::Itertools;
use sea_orm::{ActiveValue, prelude::BigDecimal};
use std::str::FromStr;

use crate::message_buffer::{Consolidate, ConsolidatedMessage, Key};

use super::types::{
    AnnotatedEvent, CallOutcome, Message, MessageExecutionOutcome, MessageId, SentOrRouted,
    SentOrRoutedAndCalled, TokenTransfer,
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
        let source_data = match (&self.send, self.source_chain_is_unknown) {
            // Case 1: Have send event - use it (normal path).
            (Some(send), _) => SourceData::from_send(send)?,

            // Case 2: No send, source is UNKNOWN - fall back to receive/execution.
            (None, true) => match (&self.receive, &self.execution) {
                (Some(receive), _) => SourceData::from_receive(receive)?,
                (None, Some(exec)) => SourceData::from_execution(exec)?,
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

        let is_ictt_complete = match &self.transfer {
            None => true, // No ICTT - not applicable
            Some(TokenTransfer::Sent(src, dst)) => src.is_some() && dst.is_some(),
            Some(TokenTransfer::SentAndCalled(src, dst)) => src.is_some() && dst.is_some(),
        };

        let is_execution_succeeded =
            matches!(self.execution, Some(MessageExecutionOutcome::Succeeded(_)));

        // Message is final when:
        // - Execution succeeded (MessageExecuted received), AND
        // - ICTT transfer is complete (if applicable)
        // Failed messages are NOT final - they can be retried via retryMessageExecution()
        let is_final = is_execution_succeeded && is_ictt_complete;

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

        // Build transfers from ICTT events if present.
        // If transfer building fails (e.g., BigDecimal parsing), propagate the error.
        let transfers = if let Some(send) = self.send.as_ref()
            && let Some(transfer) = self.transfer.as_ref()
        {
            vec![build_transfer(transfer, key, send)?]
        } else {
            Vec::new()
        };

        Ok(Some(ConsolidatedMessage {
            is_final,
            message,
            transfers,
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

    match transfer {
        TokenTransfer::Sent(src, dest) => {
            // This should never happen because we cannot call this function without a
            // send event. And if there were sent event, src must be Some.
            let src = src.as_ref().context("missing source side of a transfer")?;
            let (sender, amount, dst_token_addr, recipient, src_token_addr) = match src {
                SentOrRouted::Sent(e) => (
                    e.event.sender,
                    e.event.amount,
                    e.event.input.destinationTokenTransferrerAddress,
                    e.event.input.recipient,
                    e.contract_address,
                ),
                SentOrRouted::Routed(e) => (
                    alloy::primitives::Address::ZERO, // Routed doesn't have sender
                    e.event.amount,
                    e.event.input.destinationTokenTransferrerAddress,
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
                src_amount: ActiveValue::Set(BigDecimal::from_str(&amount.to_string())?),
                dst_amount: ActiveValue::Set(BigDecimal::from_str(&amount.to_string())?),
                token_src_address: ActiveValue::Set(src_token_addr.as_slice().to_vec()),
                token_dst_address: ActiveValue::Set(dst_token_addr.as_slice().to_vec()),
                recipient_address: ActiveValue::Set(recipient_address.as_slice().to_vec().into()),
                ..Default::default()
            };

            Ok(model)
        }
        TokenTransfer::SentAndCalled(src, dest) => {
            let src = src.as_ref().context("missing source side of a transfer")?;
            // Fill from source event
            let (sender, amount, dst_token_addr, recipient, fallback, src_token_addr) = match src {
                SentOrRoutedAndCalled::Sent(e) => (
                    e.event.sender,
                    e.event.amount,
                    e.event.input.destinationTokenTransferrerAddress,
                    e.event.input.recipientContract,
                    e.event.input.fallbackRecipient,
                    e.contract_address,
                ),
                SentOrRoutedAndCalled::Routed(e) => (
                    alloy::primitives::Address::ZERO,
                    e.event.amount,
                    e.event.input.destinationTokenTransferrerAddress,
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
                src_amount: ActiveValue::Set(BigDecimal::from_str(&amount.to_string())?),
                dst_amount: ActiveValue::Set(BigDecimal::from_str(&amount.to_string())?),
                token_src_address: ActiveValue::Set(src_token_addr.as_slice().to_vec()),
                token_dst_address: ActiveValue::Set(dst_token_addr.as_slice().to_vec()),
                // If call failed, use fallback recipient
                recipient_address: ActiveValue::Set(recipient_address.as_slice().to_vec().into()),
                ..Default::default()
            };

            Ok(model)
        }
    }
}
