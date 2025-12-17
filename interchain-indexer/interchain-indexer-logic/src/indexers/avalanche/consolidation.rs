use anyhow::Result;
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers, sea_orm_active_enums::MessageStatus,
};
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
        let send_event = &send.event;

        // Determine status based on execution outcome
        let status = match &self.execution {
            Some(MessageExecutionOutcome::Succeeded(_)) => MessageStatus::Completed,
            Some(MessageExecutionOutcome::Failed(_)) => MessageStatus::Failed,
            None => MessageStatus::Initiated,
        };

        // Get destination-side info from receive event or execution event
        let (destination_chain_id, destination_transaction_hash, last_update_timestamp) =
            if let Some(receive) = &self.receive {
                (
                    receive.chain_id.into(),
                    receive.transaction_hash.as_slice().to_vec().into(),
                    receive.block_timestamp.into(),
                )
            } else {
                (None, None, None)
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
            src_chain_id: ActiveValue::Set(send.chain_id),
            dst_chain_id: ActiveValue::Set(destination_chain_id),
            native_id: ActiveValue::Set(Some(send_event.messageID.as_slice().to_vec())),
            init_timestamp: ActiveValue::Set(send.block_timestamp),
            last_update_timestamp: ActiveValue::Set(last_update_timestamp),
            src_tx_hash: ActiveValue::Set(Some(send.transaction_hash.as_slice().to_vec())),
            dst_tx_hash: ActiveValue::Set(destination_transaction_hash),
            sender_address: ActiveValue::Set(Some(
                send_event.message.originSenderAddress.as_slice().to_vec(),
            )),
            recipient_address: ActiveValue::Set(Some(
                send_event.message.destinationAddress.as_slice().to_vec(),
            )),
            payload: ActiveValue::Set(Some(send_event.message.message.to_vec())),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
        };

        // Build transfers from ICTT events if present.
        // If transfer building fails (e.g., BigDecimal parsing), propagate the error.
        let transfers = self
            .transfer
            .as_ref()
            .map(|t| build_transfer(t, key, send.chain_id, destination_chain_id))
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
    dest_chain_id: Option<i64>,
) -> Result<crosschain_transfers::ActiveModel> {
    // Default decimals - in the future this should be fetched from token contract
    const DEFAULT_DECIMALS: i16 = 18;

    match transfer {
        TokenTransfer::Sent(src, dest) => {
            let mut transfer = crosschain_transfers::ActiveModel {
                message_id: ActiveValue::Set(key.message_id),
                bridge_id: ActiveValue::Set(key.bridge_id),
                // Always 0 for ICTT transfers
                index: ActiveValue::Set(0),
                // Set required chain ID fields
                token_src_chain_id: ActiveValue::Set(src_chain_id),
                token_dst_chain_id: ActiveValue::Set(dest_chain_id.unwrap_or(src_chain_id)),
                // Default decimals - should be fetched from token contract in the future
                src_decimals: ActiveValue::Set(DEFAULT_DECIMALS),
                dst_decimals: ActiveValue::Set(DEFAULT_DECIMALS),
                // Default empty token address - will be set from source event if available
                token_src_address: ActiveValue::Set(Vec::new()),
                ..Default::default()
            };

            // Fill from source event
            if let Some(src_event) = src {
                let (sender, amount, dst_token_addr, recipient, src_token_addr) = match src_event {
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
                transfer.sender_address = ActiveValue::Set(Some(sender.as_slice().to_vec()));
                transfer.src_amount =
                    ActiveValue::Set(BigDecimal::from_str(&amount.to_string()).unwrap_or_default());
                transfer.dst_amount =
                    ActiveValue::Set(BigDecimal::from_str(&amount.to_string()).unwrap_or_default());
                transfer.token_src_address = ActiveValue::Set(src_token_addr.as_slice().to_vec());
                transfer.token_dst_address = ActiveValue::Set(dst_token_addr.as_slice().to_vec());
                transfer.recipient_address = ActiveValue::Set(Some(recipient.as_slice().to_vec()));
            }

            // Fill from destination event
            if let Some(dst_event) = dest {
                transfer.recipient_address =
                    ActiveValue::Set(Some(dst_event.recipient.as_slice().to_vec()));
            }

            Ok(transfer)
        }
        TokenTransfer::SentAndCalled(src, dest) => {
            let mut transfer = crosschain_transfers::ActiveModel {
                message_id: ActiveValue::Set(key.message_id),
                bridge_id: ActiveValue::Set(key.bridge_id),
                index: ActiveValue::Set(0),
                // Set required chain ID fields
                token_src_chain_id: ActiveValue::Set(src_chain_id),
                token_dst_chain_id: ActiveValue::Set(dest_chain_id.unwrap_or(src_chain_id)),
                // Default decimals - should be fetched from token contract in the future
                src_decimals: ActiveValue::Set(DEFAULT_DECIMALS),
                dst_decimals: ActiveValue::Set(DEFAULT_DECIMALS),
                // Default empty token address - will be set from source event
                // if available
                ..Default::default()
            };

            // Fill from source event
            if let Some(src_event) = src {
                let (sender, amount, dst_token_addr, recipient, fallback, src_token_addr) =
                    match src_event {
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
                transfer.sender_address = ActiveValue::Set(Some(sender.as_slice().to_vec()));
                transfer.src_amount = ActiveValue::Set(BigDecimal::from_str(&amount.to_string())?);
                transfer.dst_amount = ActiveValue::Set(BigDecimal::from_str(&amount.to_string())?);
                transfer.token_src_address = ActiveValue::Set(src_token_addr.as_slice().to_vec());
                transfer.token_dst_address = ActiveValue::Set(dst_token_addr.as_slice().to_vec());

                // If call failed, use fallback recipient
                let final_recipient = match dest {
                    Some(CallOutcome::Failed(_)) => fallback,
                    _ => recipient,
                };
                transfer.recipient_address =
                    ActiveValue::Set(Some(final_recipient.as_slice().to_vec()));
            }

            Ok(transfer)
        }
    }
}
