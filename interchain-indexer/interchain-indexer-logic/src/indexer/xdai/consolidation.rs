use std::str::FromStr;

use alloy::primitives::{Address, U256};
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use interchain_indexer_entity::{
    amb_messages_confirmations, crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{MessageStatus, TransferType},
};
use sea_orm::{ActiveValue, prelude::BigDecimal};

use crate::message_buffer::{Consolidate, ConsolidatedMessage, Key};

use super::{
    types::{Direction, Message, NATIVE_SENTINEL, native_id_blob},
    version::DAI,
};

impl Consolidate for Message {
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>> {
        // Mirrors the AMB `pending_messages` pattern (and the Avalanche
        // `SourceData` gate): without a source event there is no recipient,
        // no timestamp to anchor `init_timestamp` on, and no direction to
        // derive `native_id` from. The buffer keeps whatever destination-side
        // evidence has arrived and retries once the source event lands.
        //
        // `source_request` takes priority when (hypothetically) both were
        // somehow set on one entry: a buffer key is derived from one
        // direction's nonce/messageHash space, so in practice at most one of
        // the two is ever populated for a given key.
        let (direction, nonce, recipient, src_tx_hash, init_timestamp) =
            match (&self.source_request, &self.signature_request) {
                (Some(source), _) => (
                    Direction::EthToGno,
                    source.event.nonce,
                    source.event.recipient,
                    source.transaction_hash,
                    source.block_timestamp,
                ),
                (None, Some(signature_request)) => (
                    Direction::GnoToEth,
                    signature_request.event.nonce,
                    signature_request.event.recipient,
                    signature_request.transaction_hash,
                    signature_request.block_timestamp,
                ),
                (None, None) => return Ok(None),
            };

        let native_id = native_id_blob(direction.initiator_chain_id(), nonce)?;
        let (status, last_update_timestamp, dst_tx_hash, is_final) =
            status_and_finality(direction, self);
        let transfer = build_transfer(key, direction, self)?;

        let message_model = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            status: ActiveValue::Set(status),
            init_timestamp: ActiveValue::Set(init_timestamp),
            last_update_timestamp: ActiveValue::Set(last_update_timestamp),
            src_chain_id: ActiveValue::Set(direction.initiator_chain_id()),
            dst_chain_id: ActiveValue::Set(Some(direction.destination_chain_id())),
            native_id: ActiveValue::Set(Some(native_id.to_vec())),
            src_tx_hash: ActiveValue::Set(Some(src_tx_hash.as_slice().to_vec())),
            dst_tx_hash: ActiveValue::Set(dst_tx_hash),
            sender_address: ActiveValue::Set(self.sender_address.map(|a| a.as_slice().to_vec())),
            recipient_address: ActiveValue::Set(Some(recipient.as_slice().to_vec())),
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
            transfers: vec![transfer],
            amb_confirmations,
            // The nonce is contract-issued, so a duplicate key can only be an
            // indexing bug; there is no protocol-level collision to record.
            amb_anomalies: Vec::new(),
        }))
    }
}

/// `(direction, destination_execution)` is the whole contract, mirroring
/// `amb/consolidation.rs::status_and_finality` exactly. The
/// `(GnoToEth, None) if signatures_collected.is_some()` guard is
/// load-bearing, not decorative: `handle_collected_signatures` correlates
/// purely by `messageHash` with no direction check of its own, so this term
/// is the only thing stopping a `CollectedSignatures` whose hash resolved
/// onto an Eth→Gno key from marking that message `ReadyToClaim` — do not
/// "simplify" it to an evidence-only guard. See
/// `xdai_ready_to_claim_requires_gno_to_eth_direction_even_if_signatures_collected_is_set`.
fn status_and_finality(
    direction: Direction,
    message: &Message,
) -> (MessageStatus, Option<NaiveDateTime>, Option<Vec<u8>>, bool) {
    match (direction, &message.destination_execution) {
        (_, Some(completion)) => {
            let event = completion.event();
            (
                MessageStatus::Completed,
                Some(event.block_timestamp),
                Some(event.transaction_hash.as_slice().to_vec()),
                true,
            )
        }
        (Direction::GnoToEth, None) if message.signatures_collected.is_some() => {
            let event = message
                .signatures_collected
                .as_ref()
                .expect("checked is_some");
            // `CollectedSignatures` is emitted on the *source* (Gnosis)
            // chain, not a destination transaction, so `dst_tx_hash` stays
            // `None` until `RelayedMessage` actually executes on Ethereum.
            (
                MessageStatus::ReadyToClaim,
                Some(event.block_timestamp),
                None,
                false,
            )
        }
        // No `Failed` arm: neither destination event carries a status flag,
        // and a failed execution reverts without emitting.
        _ => (MessageStatus::Initiated, None, None, false),
    }
}

/// Exactly one transfer row per message (`index = 0`): there is no payload
/// field and `isMessageValid` constrains the message blob to {104, 124}
/// bytes, so a bridge message never carries more than one movement.
///
/// `src_amount` comes from the source event's `value`; `dst_amount` comes
/// from the destination event's own `value` once that event is observed, and
/// falls back to the source value only while the message is still in flight.
/// Neither side is ever mirrored over an *observed* value — see the inline
/// note in the body for why that distinction matters here.
fn build_transfer(
    key: &Key,
    direction: Direction,
    message: &Message,
) -> Result<crosschain_transfers::ActiveModel> {
    let (recipient, value, token_src_address, token_dst_address, transfer_type) =
        match (&message.source_request, &message.signature_request) {
            (Some(source), _) => (
                source.event.recipient,
                source.event.value,
                source.event.source_asset,
                NATIVE_SENTINEL,
                TransferType::Erc20ToNative,
            ),
            (None, Some(signature_request)) => (
                signature_request.event.recipient,
                signature_request.event.value,
                NATIVE_SENTINEL,
                // Explicit only from Home v7; the legacy 104-byte layout
                // (Home v6) hardcodes DAI (`parseMessage`).
                signature_request.event.token.unwrap_or(DAI),
                TransferType::NativeToErc20,
            ),
            (None, None) => unreachable!("consolidate() returns early without a source event"),
        };

    // `dst_amount` comes from the destination event's own `value` whenever
    // that event has been observed, and falls back to the source value only
    // for a message still in flight.
    //
    // Both destination events carry the credited/paid-out amount
    // (`AffirmationCompleted(recipient, value, nonce)`,
    // `RelayedMessage(recipient, value, nonce)`), so this side is genuinely
    // observable -- mirroring the source value over it would be the ADR-003
    // "never mirror" trap applied to a side that is *not* unknown. Today the
    // two are byte-identical (no fee manager is configured, verified in
    // `.memory-bank/research/xdai-bridge-protocol-and-indexing-fit.md`), so
    // this changes no output; it is what keeps `dst_amount` correct if a fee
    // is ever activated, which the research note lists as a change trigger.
    // Reading the mirrored value instead would make that divergence
    // undetectable, since `FeeDistributedFrom*` is deliberately not
    // subscribed.
    let src_amount = amount_to_decimal(value)?;
    let dst_amount = match &message.destination_execution {
        Some(completion) => amount_to_decimal(completion.event().event.value)?,
        None => src_amount.clone(),
    };

    Ok(crosschain_transfers::ActiveModel {
        message_id: ActiveValue::Set(key.message_id),
        bridge_id: ActiveValue::Set(key.bridge_id as i32),
        index: ActiveValue::Set(0),
        r#type: ActiveValue::Set(Some(transfer_type)),
        token_src_chain_id: ActiveValue::Set(direction.initiator_chain_id()),
        token_dst_chain_id: ActiveValue::Set(direction.destination_chain_id()),
        src_amount: ActiveValue::Set(Some(src_amount)),
        dst_amount: ActiveValue::Set(Some(dst_amount)),
        token_src_address: ActiveValue::Set(Some(address_bytes(token_src_address))),
        token_dst_address: ActiveValue::Set(Some(address_bytes(token_dst_address))),
        sender_address: ActiveValue::Set(message.sender_address.map(address_bytes)),
        recipient_address: ActiveValue::Set(Some(address_bytes(recipient))),
        token_ids: ActiveValue::Set(None),
        stats_processed: ActiveValue::Set(0),
        stats_asset_id: ActiveValue::Set(None),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
        id: ActiveValue::NotSet,
    })
}

fn address_bytes(address: Address) -> Vec<u8> {
    address.as_slice().to_vec()
}

fn amount_to_decimal(amount: U256) -> Result<BigDecimal> {
    BigDecimal::from_str(&amount.to_string())
        .with_context(|| format!("failed to parse xDai transfer amount {amount}"))
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, B256, U256};
    use chrono::{DateTime, NaiveDateTime};
    use interchain_indexer_entity::sea_orm_active_enums::MessageStatus;
    use sea_orm::ActiveValue;

    use super::*;
    use crate::indexer::xdai::types::{
        AnnotatedEvent, CollectedSignaturesEvent, Completion, CompletionEvent,
        UserRequestForAffirmationEvent, UserRequestForSignatureEvent, ValidatorConfirmation,
        key_from_native_id,
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
                    source_asset: addr(0xDA),
                },
                transaction_hash: hash(0x11),
                block_number: 10,
                block_timestamp: block_ts,
            }),
            sender_address: Some(addr(0x55)),
            ..Default::default()
        }
    }

    fn signature_request(nonce: u64, recipient: Address, block_ts: NaiveDateTime) -> Message {
        Message {
            direction: Some(Direction::GnoToEth),
            signature_request: Some(AnnotatedEvent {
                event: UserRequestForSignatureEvent {
                    recipient,
                    value: U256::from(2_000u64),
                    nonce: U256::from(nonce),
                    token: None,
                },
                transaction_hash: hash(0x66),
                block_number: 30,
                block_timestamp: block_ts,
            }),
            sender_address: Some(addr(0x77)),
            ..Default::default()
        }
    }

    #[test]
    fn consolidate_without_source_request_is_not_yet_consolidatable() {
        let message = Message {
            destination_execution: Some(Completion::Affirmation(AnnotatedEvent {
                event: CompletionEvent {
                    recipient: addr(2),
                    value: U256::from(1_000u64),
                },
                transaction_hash: hash(0x22),
                block_number: 20,
                block_timestamp: ts(2_000),
            })),
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
        message.destination_execution = Some(Completion::Affirmation(AnnotatedEvent {
            event: CompletionEvent {
                recipient: addr(2),
                value: U256::from(1_000u64),
            },
            transaction_hash: hash(0x22),
            block_number: 20,
            block_timestamp: ts(2_000),
        }));
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

        assert_eq!(
            consolidated.transfers.len(),
            1,
            "exactly one row, index = 0"
        );
        let t = &consolidated.transfers[0];
        assert_eq!(set_value!(t.index), 0);
        assert_eq!(set_value!(t.r#type), Some(TransferType::Erc20ToNative));
        assert_eq!(
            set_value!(t.token_src_address),
            Some(addr(0xDA).as_slice().to_vec())
        );
        assert_eq!(
            set_value!(t.token_dst_address),
            Some(NATIVE_SENTINEL.as_slice().to_vec())
        );
        assert_eq!(set_value!(t.src_amount), Some(BigDecimal::from(1_000)));
        assert_eq!(set_value!(t.dst_amount), Some(BigDecimal::from(1_000)));
    }

    /// `dst_amount` must come from the destination event's own `value`, not
    /// be mirrored from the source. The two are byte-identical today because
    /// no fee manager is configured, so only a deliberately divergent pair
    /// distinguishes the two implementations — and if `dst_amount` is ever
    /// mirrored again, a fee becoming active would silently overstate what
    /// the recipient received, with `FeeDistributedFrom*` unsubscribed and
    /// therefore no other detector.
    #[test]
    fn consolidate_takes_dst_amount_from_the_destination_event_not_the_source() {
        let mut message = source_request(0x1ae0, addr(2), ts(1_000));
        // Source 1000, destination 995: the shape a 0.5 % fee would produce.
        message.destination_execution = Some(Completion::Affirmation(AnnotatedEvent {
            event: CompletionEvent {
                recipient: addr(2),
                value: U256::from(995u64),
            },
            transaction_hash: hash(0x22),
            block_number: 20,
            block_timestamp: ts(2_000),
        }));
        let key =
            key_from_native_id(&native_id_blob(1, U256::from(0x1ae0_u64)).unwrap(), 3).unwrap();

        let consolidated = message.consolidate(&key).unwrap().unwrap();
        let t = &consolidated.transfers[0];

        assert_eq!(set_value!(t.src_amount), Some(BigDecimal::from(1_000)));
        assert_eq!(
            set_value!(t.dst_amount),
            Some(BigDecimal::from(995)),
            "dst_amount must be the destination event's value, not the source's"
        );
    }

    /// The in-flight case: with no destination event yet, `dst_amount` falls
    /// back to the source value rather than being left NULL — the bridge has
    /// already committed to `(recipient, value, nonce)`, so the expected
    /// payout is known even before it executes.
    #[test]
    fn consolidate_falls_back_to_the_source_amount_while_the_message_is_in_flight() {
        let message = source_request(0x1ae1, addr(2), ts(1_000));
        let key =
            key_from_native_id(&native_id_blob(1, U256::from(0x1ae1_u64)).unwrap(), 3).unwrap();

        let consolidated = message.consolidate(&key).unwrap().unwrap();
        let t = &consolidated.transfers[0];

        assert_eq!(set_value!(t.src_amount), Some(BigDecimal::from(1_000)));
        assert_eq!(set_value!(t.dst_amount), Some(BigDecimal::from(1_000)));
    }

    #[test]
    fn consolidate_gno_to_eth_source_only_message_is_initiated() {
        let message = signature_request(0x140a, addr(3), ts(3_000));
        let key =
            key_from_native_id(&native_id_blob(100, U256::from(0x140a_u64)).unwrap(), 3).unwrap();

        let consolidated = message.consolidate(&key).unwrap().unwrap();

        assert!(!consolidated.is_final);
        let m = &consolidated.message;
        assert_eq!(set_value!(m.status), MessageStatus::Initiated);
        assert_eq!(set_value!(m.src_chain_id), 100);
        assert_eq!(set_value!(m.dst_chain_id), Some(1));
        assert_eq!(
            set_value!(m.sender_address),
            Some(addr(0x77).as_slice().to_vec())
        );
        assert_eq!(
            set_value!(m.recipient_address),
            Some(addr(3).as_slice().to_vec())
        );
    }

    #[test]
    fn consolidate_gno_to_eth_with_signatures_collected_is_ready_to_claim_with_no_dst_tx_hash() {
        let mut message = signature_request(0x140a, addr(3), ts(3_000));
        message.signatures_collected = Some(AnnotatedEvent {
            event: CollectedSignaturesEvent {
                authority_responsible_for_relay: addr(4),
                message_hash: hash(0x88),
                count: U256::from(4u64),
            },
            transaction_hash: hash(0x99),
            block_number: 40,
            block_timestamp: ts(3_500),
        });
        let key =
            key_from_native_id(&native_id_blob(100, U256::from(0x140a_u64)).unwrap(), 3).unwrap();

        let consolidated = message.consolidate(&key).unwrap().unwrap();

        assert!(!consolidated.is_final);
        let m = &consolidated.message;
        assert_eq!(set_value!(m.status), MessageStatus::ReadyToClaim);
        assert_eq!(set_value!(m.dst_tx_hash), None);
        assert_eq!(set_value!(m.last_update_timestamp), Some(ts(3_500)));
    }

    #[test]
    fn consolidate_gno_to_eth_relayed_is_completed_and_final() {
        let mut message = signature_request(0x140a, addr(3), ts(3_000));
        message.destination_execution = Some(Completion::Relayed(AnnotatedEvent {
            event: CompletionEvent {
                recipient: addr(3),
                value: U256::from(2_000u64),
            },
            transaction_hash: hash(0xAA),
            block_number: 50,
            block_timestamp: ts(4_000),
        }));
        let key =
            key_from_native_id(&native_id_blob(100, U256::from(0x140a_u64)).unwrap(), 3).unwrap();

        let consolidated = message.consolidate(&key).unwrap().unwrap();

        assert!(consolidated.is_final);
        assert_eq!(
            set_value!(consolidated.message.status),
            MessageStatus::Completed
        );
        assert_eq!(
            set_value!(consolidated.message.dst_tx_hash),
            Some(hash(0xAA).as_slice().to_vec())
        );

        let t = &consolidated.transfers[0];
        assert_eq!(set_value!(t.r#type), Some(TransferType::NativeToErc20));
        assert_eq!(
            set_value!(t.token_src_address),
            Some(NATIVE_SENTINEL.as_slice().to_vec())
        );
        // `signature_request`'s fixture leaves `token: None` (Home v6), so
        // this pins the legacy layout's hardcoded-DAI fallback.
        assert_eq!(
            set_value!(t.token_dst_address),
            Some(DAI.as_slice().to_vec())
        );
    }

    /// Home v7's explicit `token` field must be used verbatim, never
    /// defaulted to DAI.
    #[test]
    fn consolidate_gno_to_eth_uses_the_explicit_home_v7_token_when_present() {
        let mut message = signature_request(0x140a, addr(3), ts(3_000));
        message.signature_request.as_mut().unwrap().event.token = Some(addr(0x55));
        let key =
            key_from_native_id(&native_id_blob(100, U256::from(0x140a_u64)).unwrap(), 3).unwrap();

        let consolidated = message.consolidate(&key).unwrap().unwrap();

        let t = &consolidated.transfers[0];
        assert_eq!(set_value!(t.r#type), Some(TransferType::NativeToErc20));
        assert_eq!(
            set_value!(t.token_src_address),
            Some(NATIVE_SENTINEL.as_slice().to_vec())
        );
        assert_eq!(
            set_value!(t.token_dst_address),
            Some(addr(0x55).as_slice().to_vec())
        );
    }

    /// The regression this pins: `handle_collected_signatures` correlates
    /// purely by `messageHash`, so if one ever resolved onto an Eth→Gno
    /// key, `signatures_collected` would end up set on a message whose
    /// `direction` is `EthToGno`. The direction term in
    /// `status_and_finality`'s match pattern is what stops that from
    /// producing `ReadyToClaim` -- an evidence-only guard
    /// (`signatures_collected.is_some()` alone) would not.
    #[test]
    fn xdai_ready_to_claim_requires_gno_to_eth_direction_even_if_signatures_collected_is_set() {
        let mut message = source_request(0x1adf, addr(2), ts(1_000));
        // Simulates a `CollectedSignatures` whose hash misresolved onto this
        // Eth→Gno message's key.
        message.signatures_collected = Some(AnnotatedEvent {
            event: CollectedSignaturesEvent {
                authority_responsible_for_relay: addr(4),
                message_hash: hash(0x88),
                count: U256::from(4u64),
            },
            transaction_hash: hash(0x99),
            block_number: 40,
            block_timestamp: ts(1_500),
        });
        let key =
            key_from_native_id(&native_id_blob(1, U256::from(0x1adf_u64)).unwrap(), 3).unwrap();

        let consolidated = message.consolidate(&key).unwrap().unwrap();

        assert_ne!(
            set_value!(consolidated.message.status),
            MessageStatus::ReadyToClaim,
            "an Eth→Gno message must never become ReadyToClaim, even if \
             signatures_collected is (erroneously) set on it"
        );
    }
}
