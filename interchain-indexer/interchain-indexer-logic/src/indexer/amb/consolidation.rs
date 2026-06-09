use std::{str::FromStr, time::Duration};

use anyhow::{Context, Result};
use interchain_indexer_entity::{
    amb_message_anomalies, amb_messages_confirmations, crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{MessageStatus, TransferType},
};
use sea_orm::{ActiveValue, prelude::BigDecimal};

use crate::message_buffer::{Consolidate, ConsolidatedMessage, Key};

use super::{
    metrics::AMB_IDENTITY_CONFLICTS_TOTAL,
    types::{
        AnnotatedEvent, DestinationExecution, DestinationExecutionEvent,
        DestinationTransferDetails, Direction, Message, SourceRequest, SourceRequestEvent,
        SourceTransferDetails,
    },
};

impl Consolidate for Message {
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>> {
        let source = self.source_request.as_ref().map(SourceRequest::event);
        let destination_execution = self.destination_execution.as_ref();

        // Consolidation is the single point where both halves of a key are
        // visible, so it is where a `messageId` collision (two different bodies
        // sharing a structured `messageId`) is detected and split.
        let mut consolidated = match (source, destination_execution) {
            (Some(source), Some(destination_execution)) => {
                let destination = destination_execution.event();
                if is_collision(source, destination, self.clock_skew_tolerance) {
                    // Split: the executed (destination) body wins the canonical
                    // slot; the displaced source body is captured as an anomaly.
                    record_conflict(key);
                    tracing::warn!(
                        bridge_id = key.bridge_id,
                        message_id = %source.event.message_id,
                        source_tx = %source.transaction_hash,
                        destination_tx = %destination.transaction_hash,
                        source_sender = %source.event.header.sender,
                        source_executor = %source.event.header.executor,
                        destination_sender = %destination.event.sender,
                        destination_executor = %destination.event.executor,
                        "AMB messageId collision: executed body wins canonical row, \
                         source body captured as anomaly"
                    );
                    let mut consolidated = build_destination_only(
                        destination_execution,
                        self.destination_transfer.as_ref(),
                        key,
                    )?;
                    consolidated
                        .amb_anomalies
                        .push(source_anomaly(source, destination, key));
                    consolidated
                } else {
                    build_source_led(self, source, key)?
                }
            }
            (Some(source), None) => build_source_led(self, source, key)?,
            (None, Some(destination_execution)) => build_destination_only(
                destination_execution,
                self.destination_transfer.as_ref(),
                key,
            )?,
            (None, None) => return Ok(None),
        };

        // Any second destination executions that conflicted with the canonical
        // one (captured by the handler) become additional anomaly rows.
        let canonical = destination_execution.map(DestinationExecution::event);
        for displaced in &self.displaced {
            let displaced_event = displaced.event();
            record_conflict(key);
            tracing::warn!(
                bridge_id = key.bridge_id,
                message_id = %displaced_event.event.message_id,
                destination_tx = %displaced_event.transaction_hash,
                "AMB messageId collision: second destination execution captured as anomaly"
            );
            consolidated
                .amb_anomalies
                .push(destination_anomaly(displaced_event, canonical, key));
        }

        let status_dbg = match &consolidated.message.status {
            ActiveValue::Set(status) => format!("{status:?}"),
            _ => "unset".to_string(),
        };
        let dst_tx_hash_set =
            matches!(&consolidated.message.dst_tx_hash, ActiveValue::Set(Some(_)));
        tracing::trace!(
            bridge_id = key.bridge_id,
            buffer_key = key.message_id,
            has_source = source.is_some(),
            has_destination_execution = destination_execution.is_some(),
            has_signatures_collected = self.signatures_collected.is_some(),
            direction = ?self.direction,
            displaced = self.displaced.len(),
            status = %status_dbg,
            dst_tx_hash_set,
            is_final = consolidated.is_final,
            "AMB diag: consolidated message"
        );

        Ok(Some(consolidated))
    }
}

/// A `messageId` collision: the source and destination sides belong to
/// different message bodies. Fires when their `(sender, executor)` identities
/// differ (config-independent; catches the observed incident) or when the
/// destination precedes the source by more than `clock_skew_tolerance` (an
/// impossible ordering for a genuine pair).
fn is_collision(
    source: &AnnotatedEvent<SourceRequestEvent>,
    destination: &AnnotatedEvent<DestinationExecutionEvent>,
    clock_skew_tolerance: Duration,
) -> bool {
    let header_mismatch = (source.event.header.sender, source.event.header.executor)
        != (destination.event.sender, destination.event.executor);
    let tolerance =
        chrono::TimeDelta::from_std(clock_skew_tolerance).unwrap_or(chrono::TimeDelta::MAX);
    let impossible_order = destination.block_timestamp + tolerance < source.block_timestamp;
    header_mismatch || impossible_order
}

fn record_conflict(key: &Key) {
    AMB_IDENTITY_CONFLICTS_TOTAL
        .with_label_values(&[&key.bridge_id.to_string()])
        .inc();
}

/// Build the canonical row from a (possibly partial) source-led entry. This is
/// the happy path: a source-only partial row, or a matched, non-colliding pair.
fn build_source_led(
    message: &Message,
    source: &AnnotatedEvent<SourceRequestEvent>,
    key: &Key,
) -> Result<ConsolidatedMessage> {
    let direction = message.direction.context("missing AMB direction")?;
    let (status, last_update_timestamp, dst_tx_hash, is_final) =
        status_and_finality(direction, message);

    let destination_chain_id = Some(source.destination_chain_id);
    let source_event = &source.event;
    let destination_execution = message
        .destination_execution
        .as_ref()
        .map(DestinationExecution::event);
    let recipient_address = destination_execution
        .map(|event| event.event.executor)
        .or(Some(source_event.header.executor));

    let message_model = crosschain_messages::ActiveModel {
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

    // A transfer row exists as soon as either the source-chain
    // `TokensBridgingInitiated` or the destination-chain `TokensBridged` event
    // has been observed. Pure (non-token) AMB messages produce no transfer.
    let transfers = if message.source_transfer.is_some() || message.destination_transfer.is_some() {
        vec![build_transfer(
            key,
            direction,
            source,
            message.source_transfer.as_ref(),
            message.destination_transfer.as_ref(),
        )?]
    } else {
        Vec::new()
    };

    let amb_confirmations = message
        .validator_confirmations
        .values()
        .map(|confirmation| amb_messages_confirmations::ActiveModel {
            message_id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            validator_address: ActiveValue::Set(confirmation.validator_address.as_slice().to_vec()),
            tx_hash: ActiveValue::Set(confirmation.tx_hash.as_slice().to_vec()),
            block_number: ActiveValue::Set(
                i64::try_from(confirmation.block_number).unwrap_or(i64::MAX),
            ),
            block_timestamp: ActiveValue::Set(confirmation.block_timestamp),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
        })
        .collect();

    Ok(ConsolidatedMessage {
        is_final,
        message: message_model,
        transfers,
        amb_confirmations,
        amb_anomalies: Vec::new(),
    })
}

/// Build a destination-only canonical row: an executed body with no consistent
/// source side. Sourced entirely from the destination execution (the source
/// request was either never seen or displaced by a colliding body). Always
/// `is_final` — an execution is terminal.
fn build_destination_only(
    destination_execution: &DestinationExecution,
    destination_transfer: Option<&DestinationTransferDetails>,
    key: &Key,
) -> Result<ConsolidatedMessage> {
    let destination = destination_execution.event();
    let event = &destination.event;

    let status = if event.status {
        MessageStatus::Completed
    } else {
        MessageStatus::Failed
    };
    let recipient = destination_transfer
        .map(|transfer| transfer.recipient)
        .unwrap_or(event.executor);

    let message_model = crosschain_messages::ActiveModel {
        id: ActiveValue::Set(key.message_id),
        bridge_id: ActiveValue::Set(key.bridge_id as i32),
        status: ActiveValue::Set(status),
        init_timestamp: ActiveValue::Set(destination.block_timestamp),
        last_update_timestamp: ActiveValue::Set(Some(destination.block_timestamp)),
        src_chain_id: ActiveValue::Set(destination.source_chain_id),
        dst_chain_id: ActiveValue::Set(Some(destination.destination_chain_id)),
        native_id: ActiveValue::Set(Some(event.message_id.as_slice().to_vec())),
        src_tx_hash: ActiveValue::Set(None),
        dst_tx_hash: ActiveValue::Set(Some(destination.transaction_hash.as_slice().to_vec())),
        sender_address: ActiveValue::Set(Some(event.sender.as_slice().to_vec())),
        recipient_address: ActiveValue::Set(Some(recipient.as_slice().to_vec())),
        payload: ActiveValue::Set(None),
        stats_processed: ActiveValue::Set(0),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    let transfers = match destination_transfer {
        Some(transfer) => vec![build_destination_only_transfer(transfer, destination, key)?],
        None => Vec::new(),
    };

    Ok(ConsolidatedMessage {
        is_final: true,
        message: message_model,
        transfers,
        amb_confirmations: Vec::new(),
        amb_anomalies: Vec::new(),
    })
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
        (Direction::HomeToForeign, None) if message.signatures_collected.is_some() => {
            let event = message
                .signatures_collected
                .as_ref()
                .expect("checked is_some");
            // `CollectedSignatures` is emitted on the Home chain, which for a
            // `HomeToForeign` message is the *source* chain. It marks the message
            // as claimable but is not a destination transaction, so `dst_tx_hash`
            // stays `None` until the message is actually executed on the
            // destination (`RelayedMessage`/`AffirmationCompleted`). Its timestamp
            // is still the latest known progress, so it drives `last_update_timestamp`.
            (
                MessageStatus::ReadyToClaim,
                Some(event.block_timestamp),
                None,
                false,
            )
        }
        _ => (MessageStatus::Initiated, None, None, false),
    }
}

/// Build a transfer row for a source-led message purely from the bridge's
/// token events. The source side (token, amount, sender) comes only from the
/// source-chain `TokensBridgingInitiated` event; the destination side (token,
/// amount, recipient) only from the destination-chain `TokensBridged` event.
/// Whichever event has not yet been observed leaves its columns NULL — they are
/// never mirrored from the opposite side (which would conflate, for example, a
/// source token with the destination token).
fn build_transfer(
    key: &Key,
    direction: Direction,
    source: &AnnotatedEvent<SourceRequestEvent>,
    source_transfer: Option<&SourceTransferDetails>,
    destination_transfer: Option<&DestinationTransferDetails>,
) -> Result<crosschain_transfers::ActiveModel> {
    let (token_src_chain_id, token_dst_chain_id) = match direction {
        Direction::ForeignToHome | Direction::HomeToForeign => {
            (source.source_chain_id, source.destination_chain_id)
        }
    };

    let src_amount = match source_transfer {
        Some(t) => Some(amount_to_decimal(t.amount)?),
        None => None,
    };
    let dst_amount = match destination_transfer {
        Some(t) => Some(amount_to_decimal(t.amount)?),
        None => None,
    };

    Ok(crosschain_transfers::ActiveModel {
        message_id: ActiveValue::Set(key.message_id),
        bridge_id: ActiveValue::Set(key.bridge_id as i32),
        index: ActiveValue::Set(0),
        r#type: ActiveValue::Set(Some(TransferType::Erc20)),
        token_src_chain_id: ActiveValue::Set(token_src_chain_id),
        token_dst_chain_id: ActiveValue::Set(token_dst_chain_id),
        src_amount: ActiveValue::Set(src_amount),
        dst_amount: ActiveValue::Set(dst_amount),
        token_src_address: ActiveValue::Set(source_transfer.map(|t| t.token.as_slice().to_vec())),
        token_dst_address: ActiveValue::Set(
            destination_transfer.map(|t| t.token.as_slice().to_vec()),
        ),
        sender_address: ActiveValue::Set(source_transfer.map(|t| t.sender.as_slice().to_vec())),
        recipient_address: ActiveValue::Set(
            destination_transfer.map(|t| t.recipient.as_slice().to_vec()),
        ),
        token_ids: ActiveValue::Set(None),
        stats_processed: ActiveValue::Set(0),
        stats_asset_id: ActiveValue::Set(None),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
        id: ActiveValue::NotSet,
    })
}

/// Transfer for a destination-only row: an executed body with no consistent
/// source request. Only the destination `TokensBridged` event is known, so the
/// source side (token, amount, sender) is left NULL rather than mirrored from
/// the destination.
fn build_destination_only_transfer(
    transfer: &DestinationTransferDetails,
    destination: &AnnotatedEvent<DestinationExecutionEvent>,
    key: &Key,
) -> Result<crosschain_transfers::ActiveModel> {
    Ok(crosschain_transfers::ActiveModel {
        message_id: ActiveValue::Set(key.message_id),
        bridge_id: ActiveValue::Set(key.bridge_id as i32),
        index: ActiveValue::Set(0),
        r#type: ActiveValue::Set(Some(TransferType::Erc20)),
        token_src_chain_id: ActiveValue::Set(destination.source_chain_id),
        token_dst_chain_id: ActiveValue::Set(destination.destination_chain_id),
        src_amount: ActiveValue::Set(None),
        dst_amount: ActiveValue::Set(Some(amount_to_decimal(transfer.amount)?)),
        token_src_address: ActiveValue::Set(None),
        token_dst_address: ActiveValue::Set(Some(transfer.token.as_slice().to_vec())),
        sender_address: ActiveValue::Set(None),
        recipient_address: ActiveValue::Set(Some(transfer.recipient.as_slice().to_vec())),
        token_ids: ActiveValue::Set(None),
        stats_processed: ActiveValue::Set(0),
        stats_asset_id: ActiveValue::Set(None),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
        id: ActiveValue::NotSet,
    })
}

fn amount_to_decimal(amount: alloy::primitives::U256) -> Result<BigDecimal> {
    BigDecimal::from_str(&amount.to_string())
        .with_context(|| format!("failed to parse transfer amount {amount}"))
}

/// Anomaly row for a source body displaced by a colliding executed body. The
/// full `encoded_data` is preserved for investigation; `conflict_*` records the
/// executed body that won the canonical slot.
fn source_anomaly(
    source: &AnnotatedEvent<SourceRequestEvent>,
    conflict: &AnnotatedEvent<DestinationExecutionEvent>,
    key: &Key,
) -> amb_message_anomalies::ActiveModel {
    let header = &source.event.header;
    amb_message_anomalies::ActiveModel {
        id: ActiveValue::NotSet,
        bridge_id: ActiveValue::Set(key.bridge_id as i32),
        buffer_key: ActiveValue::Set(key.message_id),
        native_id: ActiveValue::Set(source.event.message_id.as_slice().to_vec()),
        event_kind: ActiveValue::Set("source_request".to_string()),
        chain_id: ActiveValue::Set(source.source_chain_id),
        tx_hash: ActiveValue::Set(source.transaction_hash.as_slice().to_vec()),
        log_index: ActiveValue::Set(None),
        block_number: ActiveValue::Set(source.block_number),
        block_timestamp: ActiveValue::Set(source.block_timestamp),
        sender: ActiveValue::Set(Some(header.sender.as_slice().to_vec())),
        executor: ActiveValue::Set(Some(header.executor.as_slice().to_vec())),
        src_chain_id: ActiveValue::Set(Some(source.source_chain_id)),
        dst_chain_id: ActiveValue::Set(Some(source.destination_chain_id)),
        encoded_data: ActiveValue::Set(Some(source.event.encoded_data.clone())),
        conflict_sender: ActiveValue::Set(Some(conflict.event.sender.as_slice().to_vec())),
        conflict_executor: ActiveValue::Set(Some(conflict.event.executor.as_slice().to_vec())),
        conflict_tx_hash: ActiveValue::Set(Some(conflict.transaction_hash.as_slice().to_vec())),
        detail: ActiveValue::Set(Some(
            "source request displaced by executed body sharing messageId".to_string(),
        )),
        created_at: ActiveValue::NotSet,
    }
}

/// Anomaly row for a second destination execution displaced from the canonical
/// slot. `encoded_data` is unavailable (not present in a destination log), so
/// only the identity fields are captured.
fn destination_anomaly(
    displaced: &AnnotatedEvent<DestinationExecutionEvent>,
    conflict: Option<&AnnotatedEvent<DestinationExecutionEvent>>,
    key: &Key,
) -> amb_message_anomalies::ActiveModel {
    let event = &displaced.event;
    amb_message_anomalies::ActiveModel {
        id: ActiveValue::NotSet,
        bridge_id: ActiveValue::Set(key.bridge_id as i32),
        buffer_key: ActiveValue::Set(key.message_id),
        native_id: ActiveValue::Set(event.message_id.as_slice().to_vec()),
        event_kind: ActiveValue::Set("destination_execution".to_string()),
        chain_id: ActiveValue::Set(displaced.destination_chain_id),
        tx_hash: ActiveValue::Set(displaced.transaction_hash.as_slice().to_vec()),
        log_index: ActiveValue::Set(None),
        block_number: ActiveValue::Set(displaced.block_number),
        block_timestamp: ActiveValue::Set(displaced.block_timestamp),
        sender: ActiveValue::Set(Some(event.sender.as_slice().to_vec())),
        executor: ActiveValue::Set(Some(event.executor.as_slice().to_vec())),
        src_chain_id: ActiveValue::Set(Some(displaced.source_chain_id)),
        dst_chain_id: ActiveValue::Set(Some(displaced.destination_chain_id)),
        encoded_data: ActiveValue::Set(None),
        conflict_sender: ActiveValue::Set(conflict.map(|c| c.event.sender.as_slice().to_vec())),
        conflict_executor: ActiveValue::Set(conflict.map(|c| c.event.executor.as_slice().to_vec())),
        conflict_tx_hash: ActiveValue::Set(
            conflict.map(|c| c.transaction_hash.as_slice().to_vec()),
        ),
        detail: ActiveValue::Set(Some(
            "second destination execution displaced on messageId collision".to_string(),
        )),
        created_at: ActiveValue::NotSet,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use alloy::primitives::{Address, B256};
    use chrono::{DateTime, NaiveDateTime};
    use interchain_indexer_entity::sea_orm_active_enums::MessageStatus;
    use rstest::rstest;
    use sea_orm::ActiveValue;

    use alloy::primitives::U256;

    use sea_orm::prelude::BigDecimal;

    use super::is_collision;
    use crate::{
        indexer::amb::types::{
            AmbHeaderData, AnnotatedEvent, CollectedSignaturesEvent, DestinationExecution,
            DestinationExecutionEvent, DestinationTransferDetails, Direction, Message,
            SourceRequest, SourceRequestEvent, SourceTransferDetails,
        },
        message_buffer::{Consolidate, Key},
    };

    macro_rules! set_value {
        ($av:expr) => {
            match &$av {
                ActiveValue::Set(v) => v.clone(),
                other => panic!("expected ActiveValue::Set, got {other:?}"),
            }
        };
    }

    const SRC_CHAIN: i64 = 100;
    const DST_CHAIN: i64 = 200;

    fn addr(byte: u8) -> Address {
        Address::repeat_byte(byte)
    }

    fn hash(byte: u8) -> B256 {
        B256::repeat_byte(byte)
    }

    fn ts(secs: i64) -> NaiveDateTime {
        DateTime::from_timestamp(secs, 0).unwrap().naive_utc()
    }

    fn source(sender: Address, executor: Address, block_ts: NaiveDateTime) -> SourceRequest {
        SourceRequest::Signature(AnnotatedEvent {
            event: SourceRequestEvent {
                message_id: hash(0xAA),
                encoded_data: vec![1, 2, 3],
                application_calldata: vec![4, 5],
                header: AmbHeaderData {
                    message_id: hash(0xAA),
                    sender,
                    executor,
                    source_chain_id: SRC_CHAIN,
                    destination_chain_id: DST_CHAIN,
                    payload_offset: 0,
                },
            },
            transaction_hash: hash(0x11),
            block_number: 10,
            block_timestamp: block_ts,
            source_chain_id: SRC_CHAIN,
            destination_chain_id: DST_CHAIN,
        })
    }

    fn destination(
        sender: Address,
        executor: Address,
        block_ts: NaiveDateTime,
        status: bool,
        tx: u8,
    ) -> DestinationExecution {
        DestinationExecution::Relayed(AnnotatedEvent {
            event: DestinationExecutionEvent {
                sender,
                executor,
                message_id: hash(0xAA),
                status,
            },
            transaction_hash: hash(tx),
            block_number: 20,
            block_timestamp: block_ts,
            // counterpart chain is the source; emitting chain is the destination
            source_chain_id: SRC_CHAIN,
            destination_chain_id: DST_CHAIN,
        })
    }

    #[rstest]
    #[case::matching_pair_dest_after_source(
        addr(1),
        addr(2),
        addr(1),
        addr(2),
        1_000,
        2_000,
        false
    )]
    #[case::dest_before_source_within_tolerance(
        addr(1),
        addr(2),
        addr(1),
        addr(2),
        2_000,
        1_900,
        false
    )]
    #[case::dest_before_source_beyond_tolerance(
        addr(1),
        addr(2),
        addr(1),
        addr(2),
        16_000_000,
        1_000,
        true
    )]
    #[case::identity_mismatch_dest_after_source(
        addr(1),
        addr(2),
        addr(9),
        addr(2),
        1_000,
        2_000,
        true
    )]
    fn test_is_collision_cases(
        #[case] src_sender: Address,
        #[case] src_executor: Address,
        #[case] dst_sender: Address,
        #[case] dst_executor: Address,
        #[case] src_ts_secs: i64,
        #[case] dst_ts_secs: i64,
        #[case] expected: bool,
    ) {
        let SourceRequest::Signature(source) = source(src_sender, src_executor, ts(src_ts_secs))
        else {
            unreachable!()
        };
        let DestinationExecution::Relayed(dest) =
            destination(dst_sender, dst_executor, ts(dst_ts_secs), true, 0x22)
        else {
            unreachable!()
        };
        assert_eq!(
            is_collision(&source, &dest, Duration::from_secs(300)),
            expected
        );
    }

    #[test]
    fn test_consolidate_destination_only_builds_executed_canonical_row() {
        let message = Message {
            destination_execution: Some(destination(addr(1), addr(2), ts(2_000), true, 0x22)),
            ..Default::default()
        };
        let key = Key::new(42, 7);

        let consolidated = message
            .consolidate(&key)
            .unwrap()
            .expect("destination-only entry must consolidate");

        assert!(consolidated.is_final);
        assert!(consolidated.amb_anomalies.is_empty());
        let m = &consolidated.message;
        assert_eq!(set_value!(m.status), MessageStatus::Completed);
        assert_eq!(set_value!(m.src_chain_id), SRC_CHAIN);
        assert_eq!(set_value!(m.dst_chain_id), Some(DST_CHAIN));
        assert_eq!(set_value!(m.init_timestamp), ts(2_000));
        assert_eq!(set_value!(m.src_tx_hash), None);
        assert_eq!(
            set_value!(m.dst_tx_hash),
            Some(hash(0x22).as_slice().to_vec())
        );
        assert_eq!(set_value!(m.payload), None);
    }

    #[test]
    fn test_consolidate_collision_splits_into_executed_row_and_source_anomaly() {
        // Source and destination disagree on (sender, executor): a messageId
        // collision. The executed body wins; the source body is captured.
        let message = Message {
            direction: Some(Direction::HomeToForeign),
            source_request: Some(source(addr(1), addr(2), ts(2_000))),
            destination_execution: Some(destination(addr(8), addr(9), ts(3_000), true, 0x22)),
            clock_skew_tolerance: Duration::from_secs(300),
            ..Default::default()
        };
        let key = Key::new(42, 7);

        let consolidated = message.consolidate(&key).unwrap().unwrap();

        assert!(consolidated.is_final);
        // Canonical = destination-only executed row.
        let m = &consolidated.message;
        assert_eq!(set_value!(m.src_tx_hash), None);
        assert_eq!(
            set_value!(m.dst_tx_hash),
            Some(hash(0x22).as_slice().to_vec())
        );
        assert_eq!(
            set_value!(m.sender_address),
            Some(addr(8).as_slice().to_vec())
        );

        // Exactly one anomaly for the displaced source body, with full encoded_data.
        assert_eq!(consolidated.amb_anomalies.len(), 1);
        let anomaly = &consolidated.amb_anomalies[0];
        assert_eq!(set_value!(anomaly.event_kind), "source_request");
        assert_eq!(set_value!(anomaly.encoded_data), Some(vec![1, 2, 3]));
        assert_eq!(
            set_value!(anomaly.sender),
            Some(addr(1).as_slice().to_vec())
        );
        assert_eq!(
            set_value!(anomaly.conflict_sender),
            Some(addr(8).as_slice().to_vec())
        );
    }

    #[test]
    fn test_consolidate_ready_to_claim_leaves_dst_tx_hash_unset() {
        // HomeToForeign with collected signatures but no destination execution:
        // claimable, but `CollectedSignatures` is a Home/source-chain event, so it
        // must not leak into `dst_tx_hash`.
        let signatures_collected = AnnotatedEvent {
            event: CollectedSignaturesEvent {
                authority_responsible_for_relay: addr(3),
                message_hash: hash(0xCC),
                count: U256::from(1),
            },
            transaction_hash: hash(0x44),
            block_number: 15,
            block_timestamp: ts(2_500),
            // emitted on the Home chain, which is the source for HomeToForeign
            source_chain_id: SRC_CHAIN,
            destination_chain_id: DST_CHAIN,
        };
        let message = Message {
            direction: Some(Direction::HomeToForeign),
            source_request: Some(source(addr(1), addr(2), ts(2_000))),
            signatures_collected: Some(signatures_collected),
            ..Default::default()
        };
        let key = Key::new(42, 7);

        let consolidated = message.consolidate(&key).unwrap().unwrap();

        assert!(!consolidated.is_final);
        let m = &consolidated.message;
        assert_eq!(set_value!(m.status), MessageStatus::ReadyToClaim);
        assert_eq!(set_value!(m.dst_tx_hash), None);
        // Source side is still recorded.
        assert_eq!(
            set_value!(m.src_tx_hash),
            Some(hash(0x11).as_slice().to_vec())
        );
        // The signatures-collected timestamp drives last_update_timestamp.
        assert_eq!(set_value!(m.last_update_timestamp), Some(ts(2_500)));
    }

    #[test]
    fn test_consolidate_matching_pair_completes_without_anomaly() {
        // Same identity, destination after source: a genuine pair.
        let message = Message {
            direction: Some(Direction::HomeToForeign),
            source_request: Some(source(addr(1), addr(2), ts(2_000))),
            destination_execution: Some(destination(addr(1), addr(2), ts(2_100), true, 0x22)),
            clock_skew_tolerance: Duration::from_secs(300),
            ..Default::default()
        };
        let key = Key::new(42, 7);

        let consolidated = message.consolidate(&key).unwrap().unwrap();

        assert!(consolidated.is_final);
        assert!(consolidated.amb_anomalies.is_empty());
        assert_eq!(
            set_value!(consolidated.message.status),
            MessageStatus::Completed
        );
        // Source-led canonical row: source tx hash and payload preserved.
        assert!(matches!(
            consolidated.message.src_tx_hash,
            ActiveValue::Set(Some(_))
        ));
        assert!(matches!(
            consolidated.message.payload,
            ActiveValue::Set(Some(_))
        ));
    }

    #[test]
    fn test_consolidate_second_destination_execution_captured_as_anomaly() {
        let message = Message {
            destination_execution: Some(destination(addr(1), addr(2), ts(2_000), true, 0x22)),
            displaced: vec![destination(addr(8), addr(9), ts(2_500), false, 0x33)],
            ..Default::default()
        };
        let key = Key::new(42, 7);

        let consolidated = message.consolidate(&key).unwrap().unwrap();

        assert_eq!(consolidated.amb_anomalies.len(), 1);
        let anomaly = &consolidated.amb_anomalies[0];
        assert_eq!(set_value!(anomaly.event_kind), "destination_execution");
        assert_eq!(set_value!(anomaly.encoded_data), None);
        assert_eq!(
            set_value!(anomaly.sender),
            Some(addr(8).as_slice().to_vec())
        );
        // Conflict references the winning canonical execution.
        assert_eq!(
            set_value!(anomaly.conflict_sender),
            Some(addr(1).as_slice().to_vec())
        );
    }

    fn source_transfer(token: Address, amount: u64) -> SourceTransferDetails {
        SourceTransferDetails {
            token,
            sender: addr(0x1A),
            amount: U256::from(amount),
        }
    }

    fn destination_transfer(token: Address, amount: u64) -> DestinationTransferDetails {
        DestinationTransferDetails {
            token,
            recipient: addr(0x2B),
            amount: U256::from(amount),
        }
    }

    #[test]
    fn test_consolidate_transfer_full_pair_keeps_sides_distinct() {
        // Both bridge events observed: each side carries its own chain's token.
        let message = Message {
            direction: Some(Direction::HomeToForeign),
            source_request: Some(source(addr(1), addr(2), ts(2_000))),
            destination_execution: Some(destination(addr(1), addr(2), ts(2_100), true, 0x22)),
            source_transfer: Some(source_transfer(addr(0xAA), 1_000)),
            destination_transfer: Some(destination_transfer(addr(0xBB), 990)),
            clock_skew_tolerance: Duration::from_secs(300),
            ..Default::default()
        };

        let consolidated = message.consolidate(&Key::new(42, 7)).unwrap().unwrap();
        let t = &consolidated.transfers[0];

        assert_eq!(set_value!(t.token_src_address), Some(addr(0xAA).to_vec()));
        assert_eq!(set_value!(t.token_dst_address), Some(addr(0xBB).to_vec()));
        assert_eq!(set_value!(t.src_amount), Some(BigDecimal::from(1_000)));
        assert_eq!(set_value!(t.dst_amount), Some(BigDecimal::from(990)));
        assert_eq!(set_value!(t.sender_address), Some(addr(0x1A).to_vec()));
        assert_eq!(set_value!(t.recipient_address), Some(addr(0x2B).to_vec()));
    }

    #[test]
    fn test_consolidate_transfer_source_only_leaves_destination_null() {
        // Only `TokensBridgingInitiated` seen: destination side is unknown and
        // left NULL rather than mirrored from the source.
        let message = Message {
            direction: Some(Direction::HomeToForeign),
            source_request: Some(source(addr(1), addr(2), ts(2_000))),
            source_transfer: Some(source_transfer(addr(0xAA), 1_000)),
            ..Default::default()
        };

        let consolidated = message.consolidate(&Key::new(42, 7)).unwrap().unwrap();
        let t = &consolidated.transfers[0];

        assert_eq!(set_value!(t.token_src_address), Some(addr(0xAA).to_vec()));
        assert_eq!(set_value!(t.token_dst_address), None);
        assert_eq!(set_value!(t.src_amount), Some(BigDecimal::from(1_000)));
        assert_eq!(set_value!(t.dst_amount), None);
        assert_eq!(set_value!(t.recipient_address), None);
    }

    #[test]
    fn test_consolidate_transfer_destination_only_leaves_source_null() {
        // Only `TokensBridged` seen (no consistent source request): the source
        // side is unknown and left NULL, including the sender.
        let message = Message {
            destination_execution: Some(destination(addr(1), addr(2), ts(2_000), true, 0x22)),
            destination_transfer: Some(destination_transfer(addr(0xBB), 990)),
            ..Default::default()
        };

        let consolidated = message.consolidate(&Key::new(42, 7)).unwrap().unwrap();
        let t = &consolidated.transfers[0];

        assert_eq!(set_value!(t.token_src_address), None);
        assert_eq!(set_value!(t.token_dst_address), Some(addr(0xBB).to_vec()));
        assert_eq!(set_value!(t.src_amount), None);
        assert_eq!(set_value!(t.dst_amount), Some(BigDecimal::from(990)));
        assert_eq!(set_value!(t.sender_address), None);
        assert_eq!(set_value!(t.recipient_address), Some(addr(0x2B).to_vec()));
    }

    #[test]
    fn test_consolidate_without_token_events_yields_no_transfer() {
        // A pure (non-token) AMB message produces a canonical row but no transfer.
        let message = Message {
            direction: Some(Direction::HomeToForeign),
            source_request: Some(source(addr(1), addr(2), ts(2_000))),
            destination_execution: Some(destination(addr(1), addr(2), ts(2_100), true, 0x22)),
            clock_skew_tolerance: Duration::from_secs(300),
            ..Default::default()
        };

        let consolidated = message.consolidate(&Key::new(42, 7)).unwrap().unwrap();
        assert!(consolidated.transfers.is_empty());
    }
}
