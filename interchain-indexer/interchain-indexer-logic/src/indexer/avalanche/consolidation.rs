use anyhow::{Context, Result};
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers, sea_orm_active_enums::MessageStatus,
};
use itertools::Itertools;
use sea_orm::{ActiveValue, prelude::BigDecimal};
use std::str::FromStr;

use crate::message_buffer::{Consolidate, ConsolidatedMessage, Key};

use super::types::{
    CallOutcome, Message, MessageExecutionOutcome, SentOrRouted, SentOrRoutedAndCalled,
    TokenTransfer,
};

impl Consolidate for Message {
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>> {
        // Must have send event to be consolidatable (it provides init_timestamp)
        let send = match self.send.as_ref() {
            Some(s) => s,
            None => return Ok(None),
        };

        // Determine status based on execution outcome
        let status = match &self.execution {
            Some(MessageExecutionOutcome::Succeeded(_)) => MessageStatus::Completed,
            Some(MessageExecutionOutcome::Failed(_)) => MessageStatus::Failed,
            None => MessageStatus::Initiated,
        };

        let destination_chain_id = vec![
            Some(send.destination_chain_id),
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
                "destination chain id mismatch across events: {mismatch:?} (send/receive/execution must agree)"
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
            bridge_id: ActiveValue::Set(key.bridge_id),
            status: ActiveValue::Set(status),
            src_chain_id: ActiveValue::Set(send.source_chain_id),
            dst_chain_id: ActiveValue::Set(destination_chain_id.into()),
            native_id: ActiveValue::Set(Some(send.event.messageID.as_slice().to_vec())),
            init_timestamp: ActiveValue::Set(send.block_timestamp),
            last_update_timestamp: ActiveValue::Set(last_update_timestamp),
            src_tx_hash: ActiveValue::Set(Some(send.transaction_hash.as_slice().to_vec())),
            dst_tx_hash: ActiveValue::Set(destination_transaction_hash),
            sender_address: ActiveValue::Set(Some(
                send.event.message.originSenderAddress.as_slice().to_vec(),
            )),
            recipient_address: ActiveValue::Set(Some(
                send.event.message.destinationAddress.as_slice().to_vec(),
            )),
            payload: ActiveValue::Set(Some(send.event.message.message.to_vec())),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
        };

        // Build transfers from ICTT events if present.
        // If transfer building fails (e.g., BigDecimal parsing), propagate the error.
        let transfers = self
            .transfer
            .as_ref()
            .map(|t| build_transfer(t, key, send.source_chain_id, destination_chain_id))
            .transpose()?
            .map_or_else(Vec::new, |t| vec![t]);

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
    src_chain_id: i64,
    dest_chain_id: i64,
) -> Result<crosschain_transfers::ActiveModel> {
    match transfer {
        TokenTransfer::Sent(src, dest) => {
            // This should newer happen because we don't consolidate without a
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
                message_id: ActiveValue::Set(key.message_id),
                bridge_id: ActiveValue::Set(key.bridge_id),
                // Always 0 for ICTT transfers
                index: ActiveValue::Set(0),
                token_src_chain_id: ActiveValue::Set(src_chain_id),
                token_dst_chain_id: ActiveValue::Set(dest_chain_id),
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
                message_id: ActiveValue::Set(key.message_id),
                bridge_id: ActiveValue::Set(key.bridge_id),
                index: ActiveValue::Set(0),
                // Set required chain ID fields
                token_src_chain_id: ActiveValue::Set(src_chain_id),
                token_dst_chain_id: ActiveValue::Set(dest_chain_id),
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
