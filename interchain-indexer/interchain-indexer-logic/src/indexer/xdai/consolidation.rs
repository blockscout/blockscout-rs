use anyhow::Result;
use chrono::NaiveDateTime;
use interchain_indexer_entity::{
    amb_messages_confirmations, crosschain_messages, sea_orm_active_enums::MessageStatus,
};
use sea_orm::ActiveValue;

use crate::message_buffer::{Consolidate, ConsolidatedMessage, Key};

use super::types::{AffirmationCompletedEvent, AnnotatedEvent, Message, native_id_blob};

impl Consolidate for Message {
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>> {
        // Mirrors the AMB `pending_messages` pattern (and the Avalanche
        // `SourceData` gate): without the source request there is no
        // recipient, no timestamp to anchor `init_timestamp` on, and no
        // guarantee this key even belongs to the Eth→Gno flow (Gno→Eth,
        // added in a later phase, self-keys independently). The buffer keeps
        // whatever destination-side evidence has arrived and retries once
        // the source event lands.
        let Some(source) = self.source_request.as_ref() else {
            return Ok(None);
        };
        let direction = self
            .direction
            .expect("source_request is only ever set together with direction");

        let native_id = native_id_blob(direction.initiator_chain_id(), source.event.nonce)?;
        let (status, last_update_timestamp, dst_tx_hash, is_final) =
            status_and_finality(&self.destination_execution);

        let message_model = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            status: ActiveValue::Set(status),
            init_timestamp: ActiveValue::Set(source.block_timestamp),
            last_update_timestamp: ActiveValue::Set(last_update_timestamp),
            src_chain_id: ActiveValue::Set(direction.initiator_chain_id()),
            dst_chain_id: ActiveValue::Set(Some(direction.destination_chain_id())),
            native_id: ActiveValue::Set(Some(native_id.to_vec())),
            src_tx_hash: ActiveValue::Set(Some(source.transaction_hash.as_slice().to_vec())),
            dst_tx_hash: ActiveValue::Set(dst_tx_hash),
            sender_address: ActiveValue::Set(self.sender_address.map(|a| a.as_slice().to_vec())),
            recipient_address: ActiveValue::Set(Some(source.event.recipient.as_slice().to_vec())),
            payload: ActiveValue::Set(None),
            stats_processed: ActiveValue::Set(0),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
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
            replace_existing: false,
            message: message_model,
            // B3 adds the erc20_to_native transfer row.
            transfers: Vec::new(),
            amb_confirmations,
            // The nonce is contract-issued, so a duplicate key can only be an
            // indexing bug; there is no protocol-level collision to record.
            amb_anomalies: Vec::new(),
        }))
    }
}

fn status_and_finality(
    destination_execution: &Option<AnnotatedEvent<AffirmationCompletedEvent>>,
) -> (MessageStatus, Option<NaiveDateTime>, Option<Vec<u8>>, bool) {
    match destination_execution {
        Some(execution) => (
            MessageStatus::Completed,
            Some(execution.block_timestamp),
            Some(execution.transaction_hash.as_slice().to_vec()),
            true,
        ),
        // The affirmation (Eth→Gno) flow emits no threshold event at all, so
        // there is no `ReadyToClaim` window here -- see the protocol primer.
        // `Failed` is likewise unreachable: neither destination event carries
        // a status flag, and a failed execution reverts without emitting.
        None => (MessageStatus::Initiated, None, None, false),
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, B256, U256};
    use chrono::{DateTime, NaiveDateTime};
    use interchain_indexer_entity::sea_orm_active_enums::MessageStatus;
    use sea_orm::ActiveValue;

    use super::*;
    use crate::indexer::xdai::types::{
        Direction, UserRequestForAffirmationEvent, ValidatorConfirmation, key_from_native_id,
    };

    macro_rules! set_value {
        ($av:expr) => {
            match &$av {
                ActiveValue::Set(v) => v.clone(),
                other => panic!("expected ActiveValue::Set, got {other:?}"),
            }
        };
    }

    fn addr(byte: u8) -> Address {
        Address::repeat_byte(byte)
    }

    fn hash(byte: u8) -> B256 {
        B256::repeat_byte(byte)
    }

    fn ts(secs: i64) -> NaiveDateTime {
        DateTime::from_timestamp(secs, 0).unwrap().naive_utc()
    }

    fn source_request(nonce: u64, recipient: Address, block_ts: NaiveDateTime) -> Message {
        Message {
            direction: Some(Direction::EthToGno),
            source_request: Some(AnnotatedEvent {
                event: UserRequestForAffirmationEvent {
                    recipient,
                    value: U256::from(1_000u64),
                    nonce: U256::from(nonce),
                },
                transaction_hash: hash(0x11),
                block_number: 10,
                block_timestamp: block_ts,
            }),
            sender_address: Some(addr(0x55)),
            ..Default::default()
        }
    }

    #[test]
    fn consolidate_without_source_request_is_not_yet_consolidatable() {
        let message = Message {
            destination_execution: Some(AnnotatedEvent {
                event: AffirmationCompletedEvent {
                    recipient: addr(2),
                    value: U256::from(1_000u64),
                },
                transaction_hash: hash(0x22),
                block_number: 20,
                block_timestamp: ts(2_000),
            }),
            ..Default::default()
        };
        let key =
            key_from_native_id(&native_id_blob(1, U256::from(0x1adf_u64)).unwrap(), 3).unwrap();

        assert!(message.consolidate(&key).unwrap().is_none());
    }

    #[test]
    fn consolidate_source_only_message_is_initiated() {
        let message = source_request(0x1adf, addr(2), ts(1_000));
        let key =
            key_from_native_id(&native_id_blob(1, U256::from(0x1adf_u64)).unwrap(), 3).unwrap();

        let consolidated = message.consolidate(&key).unwrap().unwrap();

        assert!(!consolidated.is_final);
        let m = &consolidated.message;
        assert_eq!(set_value!(m.status), MessageStatus::Initiated);
        assert_eq!(set_value!(m.src_chain_id), 1);
        assert_eq!(set_value!(m.dst_chain_id), Some(100));
        assert_eq!(
            set_value!(m.native_id),
            Some(native_id_blob(1, U256::from(0x1adf_u64)).unwrap().to_vec())
        );
        assert_eq!(
            set_value!(m.sender_address),
            Some(addr(0x55).as_slice().to_vec())
        );
        assert_eq!(
            set_value!(m.recipient_address),
            Some(addr(2).as_slice().to_vec())
        );
        assert_eq!(set_value!(m.dst_tx_hash), None);
        assert_eq!(set_value!(m.payload), None);
    }

    #[test]
    fn consolidate_completed_message_is_final_and_carries_confirmations() {
        let mut message = source_request(0x1adf, addr(2), ts(1_000));
        message.validator_confirmations.insert(
            addr(9),
            ValidatorConfirmation {
                validator_address: addr(9),
                tx_hash: hash(0x33),
                block_number: 15,
                block_timestamp: ts(1_500),
            },
        );
        message.destination_execution = Some(AnnotatedEvent {
            event: AffirmationCompletedEvent {
                recipient: addr(2),
                value: U256::from(1_000u64),
            },
            transaction_hash: hash(0x22),
            block_number: 20,
            block_timestamp: ts(2_000),
        });
        let key =
            key_from_native_id(&native_id_blob(1, U256::from(0x1adf_u64)).unwrap(), 3).unwrap();

        let consolidated = message.consolidate(&key).unwrap().unwrap();

        assert!(consolidated.is_final);
        assert_eq!(
            set_value!(consolidated.message.status),
            MessageStatus::Completed
        );
        assert_eq!(
            set_value!(consolidated.message.dst_tx_hash),
            Some(hash(0x22).as_slice().to_vec())
        );
        assert_eq!(consolidated.amb_confirmations.len(), 1);
        assert!(consolidated.amb_anomalies.is_empty());
        assert!(consolidated.transfers.is_empty());
    }
}
