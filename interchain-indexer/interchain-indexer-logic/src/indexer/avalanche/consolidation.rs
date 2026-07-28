// SPDX-License-Identifier: LicenseRef-Blockscout

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

#[cfg(test)]
mod tests {
    use alloy::primitives::{B256, U256};

    use super::*;
    use crate::indexer::avalanche::abi::{
        ITeleporterMessenger, ITokenTransferrer, SendTokensInput, TeleporterFeeInfo,
        TeleporterMessage,
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
}
