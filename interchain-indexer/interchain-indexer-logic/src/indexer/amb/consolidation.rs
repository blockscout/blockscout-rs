use std::str::FromStr;

use anyhow::{Context, Result};
use interchain_indexer_entity::{
    amb_messages_confirmations, crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{MessageStatus, TransferType},
};
use sea_orm::{ActiveValue, prelude::BigDecimal};

use crate::message_buffer::{Consolidate, ConsolidatedMessage, Key};

use super::types::{
    DecodedPayload, DestinationExecution, Direction, Message, SourceTransferDetails,
};

impl Consolidate for Message {
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>> {
        let source = match &self.source_request {
            Some(source) => source.event(),
            None => return Ok(None),
        };

        let direction = self.direction.context("missing AMB direction")?;
        let (status, last_update_timestamp, dst_tx_hash, is_final) =
            status_and_finality(direction, self);

        let destination_chain_id = Some(source.destination_chain_id);
        let source_event = &source.event;
        let destination_execution = self
            .destination_execution
            .as_ref()
            .map(|execution| execution.event());
        let recipient_address = destination_execution
            .map(|event| event.event.executor)
            .or(Some(source_event.header.executor));

        let message = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            status: ActiveValue::Set(status),
            init_timestamp: ActiveValue::Set(source.block_timestamp),
            last_update_timestamp: ActiveValue::Set(last_update_timestamp),
            src_chain_id: ActiveValue::Set(source.source_chain_id),
            dst_chain_id: ActiveValue::Set(destination_chain_id),
            native_id: ActiveValue::Set(Some(source_event.message_id.as_slice().to_vec())),
            src_tx_hash: ActiveValue::Set(Some(source.transaction_hash.as_slice().to_vec())),
            dst_tx_hash: ActiveValue::Set(dst_tx_hash),
            sender_address: ActiveValue::Set(Some(source_event.header.sender.as_slice().to_vec())),
            recipient_address: ActiveValue::Set(recipient_address.map(|a| a.as_slice().to_vec())),
            payload: ActiveValue::Set(Some(source_event.application_calldata.clone())),
            stats_processed: ActiveValue::Set(0),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
        };

        let transfers = match &self.decoded_payload {
            Some(payload) => vec![build_transfer(
                payload,
                key,
                direction,
                source,
                self.source_transfer.as_ref(),
            )?],
            None => Vec::new(),
        };

        let amb_confirmations = self
            .validator_confirmations
            .values()
            .map(|confirmation| amb_messages_confirmations::ActiveModel {
                message_id: ActiveValue::Set(key.message_id),
                bridge_id: ActiveValue::Set(key.bridge_id as i32),
                validator_address: ActiveValue::Set(
                    confirmation.validator_address.as_slice().to_vec(),
                ),
                tx_hash: ActiveValue::Set(confirmation.tx_hash.as_slice().to_vec()),
                block_number: ActiveValue::Set(
                    i64::try_from(confirmation.block_number).unwrap_or(i64::MAX),
                ),
                block_timestamp: ActiveValue::Set(confirmation.block_timestamp),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            })
            .collect();

        Ok(Some(ConsolidatedMessage {
            is_final,
            message,
            transfers,
            amb_confirmations,
        }))
    }
}

fn status_and_finality(
    direction: Direction,
    message: &Message,
) -> (
    MessageStatus,
    Option<chrono::NaiveDateTime>,
    Option<Vec<u8>>,
    bool,
) {
    match (direction, &message.destination_execution) {
        (_, Some(DestinationExecution::Affirmation(event)))
        | (_, Some(DestinationExecution::Relayed(event))) => {
            let status = if event.event.status {
                MessageStatus::Completed
            } else {
                MessageStatus::Failed
            };
            (
                status,
                Some(event.block_timestamp),
                Some(event.transaction_hash.as_slice().to_vec()),
                true,
            )
        }
        (Direction::GnosisToEth, None) if message.signatures_collected.is_some() => {
            let event = message
                .signatures_collected
                .as_ref()
                .expect("checked is_some");
            (
                MessageStatus::ReadyToClaim,
                Some(event.block_timestamp),
                Some(event.transaction_hash.as_slice().to_vec()),
                false,
            )
        }
        _ => (MessageStatus::Initiated, None, None, false),
    }
}

fn build_transfer(
    payload: &DecodedPayload,
    key: &Key,
    direction: Direction,
    source: &super::types::AnnotatedEvent<super::types::SourceRequestEvent>,
    source_transfer: Option<&SourceTransferDetails>,
) -> Result<crosschain_transfers::ActiveModel> {
    let DecodedPayload::OmnibridgeTransfer {
        token_src_address: payload_token_src,
        token_dst_address,
        src_amount: payload_src_amount,
        dst_amount,
        sender: payload_sender,
        recipient,
    } = payload;

    let (token_src_chain_id, token_dst_chain_id) = match direction {
        Direction::EthToGnosis | Direction::GnosisToEth => {
            (source.source_chain_id, source.destination_chain_id)
        }
    };

    // The decoded application payload only carries destination-chain values
    // (mediator calldata + `TokensBridged` event). Prefer the
    // `TokensBridgingInitiated` event captured on the source chain for the
    // source-side token, sender and amount.
    let token_src = source_transfer.map_or(*payload_token_src, |t| t.token);
    let src_amount_u256 = source_transfer.map_or(*payload_src_amount, |t| t.amount);
    let sender_addr = source_transfer.map_or(*payload_sender, |t| t.sender);
    let token_dst = token_dst_address.unwrap_or(*payload_token_src);

    Ok(crosschain_transfers::ActiveModel {
        message_id: ActiveValue::Set(key.message_id),
        bridge_id: ActiveValue::Set(key.bridge_id as i32),
        index: ActiveValue::Set(0),
        r#type: ActiveValue::Set(Some(TransferType::Erc20)),
        token_src_chain_id: ActiveValue::Set(token_src_chain_id),
        token_dst_chain_id: ActiveValue::Set(token_dst_chain_id),
        src_amount: ActiveValue::Set(BigDecimal::from_str(&src_amount_u256.to_string())?),
        dst_amount: ActiveValue::Set(BigDecimal::from_str(&dst_amount.to_string())?),
        token_src_address: ActiveValue::Set(token_src.as_slice().to_vec()),
        token_dst_address: ActiveValue::Set(token_dst.as_slice().to_vec()),
        sender_address: ActiveValue::Set(Some(sender_addr.as_slice().to_vec())),
        recipient_address: ActiveValue::Set(Some(recipient.as_slice().to_vec())),
        token_ids: ActiveValue::Set(None),
        stats_processed: ActiveValue::Set(0),
        stats_asset_id: ActiveValue::Set(None),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
        id: ActiveValue::NotSet,
    })
}
