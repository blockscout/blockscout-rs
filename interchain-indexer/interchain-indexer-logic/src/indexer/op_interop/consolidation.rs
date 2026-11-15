// SPDX-License-Identifier: LicenseRef-Blockscout

use alloy::{
    hex,
    primitives::{Address, B256, U256},
};
use anyhow::Result;
use interchain_indexer_entity::{
    crosschain_messages, op_interop_legs, sea_orm_active_enums::MessageStatus,
};
use sea_orm::ActiveValue;
use serde::Serialize;

use crate::{
    message_buffer::{Consolidate, ConsolidatedMessage, Key},
    protocol_metadata::{
        OpInterop, OpInteropExecutingMessage, OpInteropIdentifier, OpInteropLegRef,
        ProtocolMetadata,
    },
};

use super::{
    metrics,
    types::{DestinationLeg, ExecutingLeg, LogRef, Message, SourceLeg},
    version::OpInteropEvent,
};

/// `consolidate` never returns `Err`: an `Err` here stalls maintenance for the
/// whole bridge on every cycle, so every conversion that can fail is done at
/// ingestion (`events::log_ref`, `events::classify_chain`).
///
/// | `source` | `destination` | Result |
/// | --- | --- | --- |
/// | none | none | not ready |
/// | none | some | not ready: the source chain is always configured, so its `SentMessage` can still arrive |
/// | some | none | `initiated`, flushed and kept |
/// | some | some | `completed`, flushed and evicted |
///
/// Waiting for the source before the first flush is what makes the row
/// well-formed: `src_tx_hash`, sender, payload and `init_timestamp` are all
/// source facts.
impl Consolidate for Message {
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>> {
        let Some(source) = &self.source else {
            return Ok(None);
        };
        let destination = self.destination.as_ref();

        let Some(rows) = StorageRows::build(source, destination) else {
            // Unreachable for an entry built by this indexer: ingestion only
            // stores values that fit the storage types. Reported instead of
            // raised, so that one malformed entry cannot stop the bridge.
            tracing::error!(
                bridge_id = key.bridge_id,
                message_id = key.message_id,
                message_hash = %source.message_hash,
                "OP interop buffer entry holds a value that does not fit its storage type; \
                 leaving it unconsolidated"
            );
            return Ok(None);
        };

        if let Some(destination) = destination
            && let Some(exec) = &destination.executing
            && !identifier_matches_source(exec, source)
        {
            metrics::OP_INTEROP_LEG_CONFLICTS_TOTAL
                .with_label_values(&[key.bridge_id.to_string().as_str(), "identifier_mismatch"])
                .inc();
            tracing::warn!(
                bridge_id = key.bridge_id,
                message_id = key.message_id,
                message_hash = %source.message_hash,
                src_tx_hash = %source.log.transaction_hash,
                dst_tx_hash = %destination.log.transaction_hash,
                "OP interop ExecutingMessage identifier does not point at the indexed \
                 SentMessage log; the message completes anyway"
            );
        }

        let (status, is_final) = match destination {
            Some(_) => (MessageStatus::Completed, true),
            None => (MessageStatus::Initiated, false),
        };

        let message = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            status: ActiveValue::Set(status),
            init_timestamp: ActiveValue::Set(source.log.block_timestamp),
            last_update_timestamp: ActiveValue::Set(
                destination.map(|destination| destination.log.block_timestamp),
            ),
            src_chain_id: ActiveValue::Set(rows.source.chain_id),
            dst_chain_id: ActiveValue::Set(Some(rows.destination_chain_id)),
            native_id: ActiveValue::Set(Some(source.message_hash.to_vec())),
            src_tx_hash: ActiveValue::Set(Some(source.log.transaction_hash.to_vec())),
            dst_tx_hash: ActiveValue::Set(
                destination.map(|destination| destination.log.transaction_hash.to_vec()),
            ),
            // The `sendMessage` caller (`msg.sender` in the messenger), not
            // the transaction origin: no transaction is fetched. It is a
            // contract for most traffic.
            sender_address: ActiveValue::Set(Some(source.sender.to_vec())),
            recipient_address: ActiveValue::Set(Some(source.target.to_vec())),
            payload: ActiveValue::Set(Some(source.message.to_vec())),
            protocol_metadata: ActiveValue::Set(
                ProtocolMetadata {
                    op_interop: Some(metadata(source, destination)),
                    ..Default::default()
                }
                .to_json_value(),
            ),
            stats_processed: ActiveValue::Set(0),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
        };

        Ok(Some(ConsolidatedMessage {
            is_final,
            replace_existing: false,
            message,
            transfers: Vec::new(),
            amb_confirmations: Vec::new(),
            amb_anomalies: Vec::new(),
            op_interop_legs: rows.leg_models(key, source, destination),
        }))
    }
}

/// A [`LogRef`] in storage types.
struct StoredLogRef {
    chain_id: i64,
    block_number: i64,
    log_index: i64,
}

impl StoredLogRef {
    fn new(log: &LogRef) -> Option<Self> {
        Some(Self {
            chain_id: i64::try_from(log.chain_id).ok()?,
            block_number: i64::try_from(log.block_number).ok()?,
            log_index: i64::try_from(log.log_index).ok()?,
        })
    }
}

/// Every storage-typed value the models need, converted up front so that the
/// rest of `consolidate` has nothing left that can fail.
struct StorageRows {
    source: StoredLogRef,
    destination_chain_id: i64,
    destination: Option<StoredLogRef>,
    executing: Option<StoredLogRef>,
}

impl StorageRows {
    fn build(source: &SourceLeg, destination: Option<&DestinationLeg>) -> Option<Self> {
        let executing = destination.and_then(|destination| destination.executing.as_ref());
        Some(Self {
            source: StoredLogRef::new(&source.log)?,
            destination_chain_id: i64::try_from(source.destination_chain_id).ok()?,
            destination: match destination {
                Some(destination) => Some(StoredLogRef::new(&destination.log)?),
                None => None,
            },
            executing: match executing {
                Some(executing) => Some(StoredLogRef::new(&executing.log)?),
                None => None,
            },
        })
    }

    /// One ledger row per log the entry holds: `sent_message`; with a
    /// destination, `relayed_message` and, if the relay was paired,
    /// `executing_message`.
    ///
    /// Every model sets the same columns and leaves `created_at` /
    /// `updated_at` unset: SeaORM cannot mix `Set` and `NotSet` for one column
    /// in one `insert_many`.
    fn leg_models(
        &self,
        key: &Key,
        source: &SourceLeg,
        destination: Option<&DestinationLeg>,
    ) -> Vec<op_interop_legs::ActiveModel> {
        let message_hash = source.message_hash;
        let mut models = vec![leg_model(
            key,
            message_hash,
            &source.log,
            &self.source,
            OpInteropEvent::SentMessage,
            SentMessageData::new(source).to_json(),
        )];

        if let (Some(destination), Some(stored)) = (destination, &self.destination) {
            if let (Some(executing), Some(stored)) = (&destination.executing, &self.executing) {
                models.push(leg_model(
                    key,
                    message_hash,
                    &executing.log,
                    stored,
                    OpInteropEvent::ExecutingMessage,
                    ExecutingMessageData::new(executing).to_json(),
                ));
            }
            models.push(leg_model(
                key,
                message_hash,
                &destination.log,
                stored,
                OpInteropEvent::RelayedMessage,
                RelayedMessageData::new(destination).to_json(),
            ));
        }

        models
    }
}

fn leg_model(
    key: &Key,
    message_hash: B256,
    log: &LogRef,
    stored: &StoredLogRef,
    event: OpInteropEvent,
    event_data: serde_json::Value,
) -> op_interop_legs::ActiveModel {
    op_interop_legs::ActiveModel {
        bridge_id: ActiveValue::Set(key.bridge_id as i32),
        chain_id: ActiveValue::Set(stored.chain_id),
        block_hash: ActiveValue::Set(log.block_hash.to_vec()),
        log_index: ActiveValue::Set(stored.log_index),
        block_number: ActiveValue::Set(stored.block_number),
        block_timestamp: ActiveValue::Set(log.block_timestamp),
        tx_hash: ActiveValue::Set(log.transaction_hash.to_vec()),
        event_kind: ActiveValue::Set(event.as_str().to_string()),
        message_id: ActiveValue::Set(key.message_id),
        message_hash: ActiveValue::Set(message_hash.to_vec()),
        event_data: ActiveValue::Set(event_data),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    }
}

/// Lower-case `0x` hex, the form every hash and address takes in
/// `event_data` and in `protocol_metadata`.
fn hex_hash(value: B256) -> String {
    hex::encode_prefixed(value)
}

fn hex_address(value: Address) -> String {
    hex::encode_prefixed(value)
}

/// `op_interop_legs.event_data` of a `sent_message` row. The message bytes
/// themselves are `crosschain_messages.payload`.
#[derive(Debug, Serialize)]
struct SentMessageData {
    destination_chain_id: String,
    nonce: String,
    sender: String,
    target: String,
    payload_hash: String,
}

impl SentMessageData {
    fn new(source: &SourceLeg) -> Self {
        Self {
            destination_chain_id: source.destination_chain_id.to_string(),
            nonce: source.nonce.to_string(),
            sender: hex_address(source.sender),
            target: hex_address(source.target),
            payload_hash: hex_hash(source.payload_hash),
        }
    }
}

/// `op_interop_legs.event_data` of a `relayed_message` row.
#[derive(Debug, Serialize)]
struct RelayedMessageData {
    source_chain_id: String,
    nonce: String,
    return_data_hash: String,
}

impl RelayedMessageData {
    fn new(destination: &DestinationLeg) -> Self {
        Self {
            source_chain_id: destination.source_chain_id.to_string(),
            nonce: destination.nonce.to_string(),
            return_data_hash: hex_hash(destination.return_data_hash),
        }
    }
}

/// `op_interop_legs.event_data` of an `executing_message` row.
#[derive(Debug, Serialize)]
struct ExecutingMessageData {
    msg_hash: String,
    identifier: OpInteropIdentifier,
}

impl ExecutingMessageData {
    fn new(executing: &ExecutingLeg) -> Self {
        Self {
            msg_hash: hex_hash(executing.msg_hash),
            identifier: identifier(executing),
        }
    }
}

/// These structs hold nothing but strings, so serializing them cannot fail;
/// `Null` is only there to keep `consolidate` free of a fallible step.
trait EventData: Serialize + Sized {
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

impl EventData for SentMessageData {}
impl EventData for RelayedMessageData {}
impl EventData for ExecutingMessageData {}

/// The on-chain `Identifier` verbatim; `uint256` members as decimal strings.
fn identifier(executing: &ExecutingLeg) -> OpInteropIdentifier {
    OpInteropIdentifier {
        origin: hex_address(executing.origin),
        chain_id: executing.source_chain_id.to_string(),
        block_number: executing.source_block_number.to_string(),
        log_index: executing.source_log_index.to_string(),
        timestamp: executing.source_timestamp.to_string(),
    }
}

fn leg_ref(log: &LogRef) -> OpInteropLegRef {
    OpInteropLegRef {
        block_number: log.block_number,
        block_hash: hex_hash(log.block_hash),
        log_index: log.log_index,
    }
}

/// The `op_interop` namespace of `protocol_metadata`: `nonce` and `source`
/// from the first flush on, `destination` once the relay is indexed, and
/// `executing_message` only for a paired relay.
fn metadata(source: &SourceLeg, destination: Option<&DestinationLeg>) -> OpInterop {
    OpInterop {
        nonce: source.nonce.to_string(),
        source: Some(leg_ref(&source.log)),
        destination: destination.map(|destination| leg_ref(&destination.log)),
        executing_message: destination
            .and_then(|destination| destination.executing.as_ref())
            .map(|executing| OpInteropExecutingMessage {
                msg_hash: hex_hash(executing.msg_hash),
                log_index: executing.log.log_index,
                identifier: identifier(executing),
            }),
    }
}

/// Diagnostic cross-check, never an outcome: does the `ExecutingMessage` a
/// relay was paired with point at the `SentMessage` log this indexer holds
/// for the same message?
///
/// `msgHash` must be the payload hash of that log, and the identifier its
/// `(chainId, blockNumber, logIndex, timestamp)`. In confirmed data a
/// mismatch can only mean a decoder or pairing bug.
pub(super) fn identifier_matches_source(executing: &ExecutingLeg, source: &SourceLeg) -> bool {
    let source_timestamp = u64::try_from(source.log.block_timestamp.and_utc().timestamp())
        .ok()
        .map(U256::from);

    executing.msg_hash == source.payload_hash
        && executing.source_chain_id == U256::from(source.log.chain_id)
        && executing.source_block_number == U256::from(source.log.block_number)
        && executing.source_log_index == U256::from(source.log.log_index)
        && Some(executing.source_timestamp) == source_timestamp
}

#[cfg(test)]
mod tests {
    use alloy::primitives::b256;
    use sea_orm::ActiveValue::Set;

    use super::{
        super::{
            events::{TxAction, plan_transaction},
            test_fixtures::{
                DESTINATION_CHAIN_ID, SOURCE_CHAIN_ID, configured_chains, registry, relay_logs,
                send_calls,
            },
            types::key_from_message_hash,
        },
        *,
    };

    const BRIDGE_ID: i32 = 2004;
    const MESSAGE_0_HASH: B256 =
        b256!("274fa1e522d77c94a725679b973bd3070ac910488a6a5cf023c1469f94655eaa");

    fn key() -> Key {
        key_from_message_hash(&MESSAGE_0_HASH, BRIDGE_ID).unwrap()
    }

    /// The source leg of message 0, as the planner builds it from the
    /// fixture.
    fn fixture_source() -> SourceLeg {
        let logs = send_calls().remove(0);
        let refs: Vec<_> = logs.iter().collect();
        let plan = plan_transaction(&registry(), &configured_chains(), SOURCE_CHAIN_ID, &refs)
            .expect("send fixture plans");
        match plan.actions.into_iter().next() {
            Some(TxAction::Send(leg)) => leg,
            other => panic!("expected a send, got {other:?}"),
        }
    }

    fn fixture_destination() -> DestinationLeg {
        let logs = relay_logs();
        let refs: Vec<_> = logs.iter().collect();
        let plan = plan_transaction(
            &registry(),
            &configured_chains(),
            DESTINATION_CHAIN_ID,
            &refs,
        )
        .expect("relay fixture plans");
        match plan.actions.into_iter().next() {
            Some(TxAction::Relay(leg)) => leg,
            other => panic!("expected a relay, got {other:?}"),
        }
    }

    fn consolidated(message: &Message) -> ConsolidatedMessage {
        message
            .consolidate(&key())
            .expect("consolidate never fails")
            .expect("entry is consolidatable")
    }

    fn value<T: Clone + std::fmt::Debug + Into<sea_orm::Value>>(active: &ActiveValue<T>) -> T {
        match active {
            Set(value) => value.clone(),
            other => panic!("expected Set, got {other:?}"),
        }
    }

    fn event_kinds(consolidated: &ConsolidatedMessage) -> Vec<String> {
        consolidated
            .op_interop_legs
            .iter()
            .map(|leg| value(&leg.event_kind))
            .collect()
    }

    #[test]
    fn test_consolidate_empty_returns_none() {
        assert!(Message::default().consolidate(&key()).unwrap().is_none());
    }

    #[test]
    fn test_consolidate_destination_only_returns_none() {
        let message = Message {
            source: None,
            destination: Some(fixture_destination()),
        };
        assert!(
            message.consolidate(&key()).unwrap().is_none(),
            "a relay alone writes nothing: it waits for its source"
        );
    }

    #[test]
    fn test_consolidate_source_only_is_initiated_and_partial() {
        let source = fixture_source();
        let message = Message {
            source: Some(source.clone()),
            destination: None,
        };

        let consolidated = consolidated(&message);

        assert!(!consolidated.is_final);
        assert!(!consolidated.replace_existing);
        assert!(consolidated.transfers.is_empty());
        assert!(consolidated.amb_confirmations.is_empty());
        assert!(consolidated.amb_anomalies.is_empty());

        let row = &consolidated.message;
        assert_eq!(value(&row.id), key().message_id);
        assert_eq!(value(&row.bridge_id), BRIDGE_ID);
        assert_eq!(value(&row.status), MessageStatus::Initiated);
        assert_eq!(value(&row.init_timestamp), source.log.block_timestamp);
        assert_eq!(value(&row.last_update_timestamp), None);
        assert_eq!(value(&row.src_chain_id), SOURCE_CHAIN_ID as i64);
        assert_eq!(value(&row.dst_chain_id), Some(DESTINATION_CHAIN_ID as i64));
        assert_eq!(value(&row.native_id), Some(MESSAGE_0_HASH.to_vec()));
        assert_eq!(
            value(&row.src_tx_hash),
            Some(source.log.transaction_hash.to_vec())
        );
        assert_eq!(value(&row.dst_tx_hash), None);
        assert_eq!(value(&row.sender_address), Some(source.sender.to_vec()));
        assert_eq!(value(&row.recipient_address), Some(source.target.to_vec()));
        assert_eq!(value(&row.payload), Some(source.message.to_vec()));
        assert_eq!(value(&row.stats_processed), 0);
        assert!(matches!(row.created_at, ActiveValue::NotSet));
        assert!(matches!(row.updated_at, ActiveValue::NotSet));

        assert_eq!(event_kinds(&consolidated), vec!["sent_message"]);
        let metadata = value(&row.protocol_metadata).expect("metadata is always written");
        assert_eq!(
            metadata,
            serde_json::json!({
                "op_interop": {
                    "nonce": "0",
                    "source": {
                        "block_number": 498922,
                        "block_hash": "0x8580247817bcfdd8f258ebd2f9a73aeddb4b87953114a723da65206fa002c8b5",
                        "log_index": 1
                    }
                }
            })
        );
    }

    #[test]
    fn test_consolidate_both_legs_is_completed_and_final() {
        let source = fixture_source();
        let destination = fixture_destination();
        let message = Message {
            source: Some(source.clone()),
            destination: Some(destination.clone()),
        };

        let consolidated = consolidated(&message);

        assert!(consolidated.is_final);
        assert!(!consolidated.replace_existing);
        let row = &consolidated.message;
        assert_eq!(value(&row.status), MessageStatus::Completed);
        assert_eq!(value(&row.init_timestamp), source.log.block_timestamp);
        assert_eq!(
            value(&row.last_update_timestamp),
            Some(destination.log.block_timestamp)
        );
        assert_eq!(
            value(&row.src_tx_hash),
            Some(source.log.transaction_hash.to_vec())
        );
        assert_eq!(
            value(&row.dst_tx_hash),
            Some(destination.log.transaction_hash.to_vec())
        );
        // Source facts are unchanged by the relay.
        assert_eq!(value(&row.src_chain_id), SOURCE_CHAIN_ID as i64);
        assert_eq!(value(&row.dst_chain_id), Some(DESTINATION_CHAIN_ID as i64));
        assert_eq!(value(&row.recipient_address), Some(source.target.to_vec()));
        assert_eq!(value(&row.stats_processed), 0);
        assert_eq!(consolidated.op_interop_legs.len(), 3);
    }

    #[test]
    fn test_consolidate_unpaired_relay_has_two_leg_rows_and_no_executing_metadata() {
        let mut destination = fixture_destination();
        destination.executing = None;
        let message = Message {
            source: Some(fixture_source()),
            destination: Some(destination),
        };

        let consolidated = consolidated(&message);

        assert!(consolidated.is_final, "an unpaired relay still completes");
        assert_eq!(
            value(&consolidated.message.status),
            MessageStatus::Completed
        );
        assert_eq!(
            event_kinds(&consolidated),
            vec!["sent_message", "relayed_message"]
        );
        let metadata = value(&consolidated.message.protocol_metadata).expect("metadata");
        let op_interop = &metadata["op_interop"];
        assert!(op_interop.get("source").is_some());
        assert!(op_interop.get("destination").is_some());
        assert!(
            op_interop.get("executing_message").is_none(),
            "absent, not null: {op_interop}"
        );
    }

    #[test]
    fn test_consolidate_leg_rows_and_metadata_match_fixture() {
        let message = Message {
            source: Some(fixture_source()),
            destination: Some(fixture_destination()),
        };

        let consolidated = consolidated(&message);

        let metadata = value(&consolidated.message.protocol_metadata).expect("metadata");
        assert_eq!(
            metadata,
            serde_json::json!({
                "op_interop": {
                    "nonce": "0",
                    "source": {
                        "block_number": 498922,
                        "block_hash": "0x8580247817bcfdd8f258ebd2f9a73aeddb4b87953114a723da65206fa002c8b5",
                        "log_index": 1
                    },
                    "destination": {
                        "block_number": 895701,
                        "block_hash": "0x7ea9e26557b70150ae7360da608f0f231e30397b4e5b4c98f4edd431d6630c73",
                        "log_index": 3
                    },
                    "executing_message": {
                        "msg_hash": "0xbb39b9f3ce34b43c1ae2bcc42aa7a2ceb451b20cdd22f2396d2ea6ce16bf31fb",
                        "log_index": 0,
                        "identifier": {
                            "origin": "0x4200000000000000000000000000000000000023",
                            "chain_id": "420130019",
                            "block_number": "498922",
                            "log_index": "1",
                            "timestamp": "1794670098"
                        }
                    }
                }
            })
        );

        let source_block =
            b256!("8580247817bcfdd8f258ebd2f9a73aeddb4b87953114a723da65206fa002c8b5").to_vec();
        let destination_block =
            b256!("7ea9e26557b70150ae7360da608f0f231e30397b4e5b4c98f4edd431d6630c73").to_vec();
        let source_tx =
            b256!("d7ff0cd43c35c6438f899a68526128c57b92995119a0900ea9493990ea8f5b9c").to_vec();
        let destination_tx =
            b256!("7155e2226f4cc4ffbb8987e756fa5e450a9c40c9e2e0dcd7f1195ae8aa58d03d").to_vec();

        struct Expected {
            kind: &'static str,
            chain_id: i64,
            block_hash: Vec<u8>,
            block_number: i64,
            timestamp: i64,
            tx_hash: Vec<u8>,
            log_index: i64,
            event_data: serde_json::Value,
        }
        let expected = [
            Expected {
                kind: "sent_message",
                chain_id: SOURCE_CHAIN_ID as i64,
                block_hash: source_block,
                block_number: 498_922,
                timestamp: 1_794_670_098,
                tx_hash: source_tx,
                log_index: 1,
                event_data: serde_json::json!({
                    "destination_chain_id": "420130020",
                    "nonce": "0",
                    "sender": "0x4200000000000000000000000000000000000024",
                    "target": "0x4200000000000000000000000000000000000024",
                    "payload_hash": "0xbb39b9f3ce34b43c1ae2bcc42aa7a2ceb451b20cdd22f2396d2ea6ce16bf31fb"
                }),
            },
            Expected {
                kind: "executing_message",
                chain_id: DESTINATION_CHAIN_ID as i64,
                block_hash: destination_block.clone(),
                block_number: 895_701,
                timestamp: 1_794_670_100,
                tx_hash: destination_tx.clone(),
                log_index: 0,
                event_data: serde_json::json!({
                    "msg_hash": "0xbb39b9f3ce34b43c1ae2bcc42aa7a2ceb451b20cdd22f2396d2ea6ce16bf31fb",
                    "identifier": {
                        "origin": "0x4200000000000000000000000000000000000023",
                        "chain_id": "420130019",
                        "block_number": "498922",
                        "log_index": "1",
                        "timestamp": "1794670098"
                    }
                }),
            },
            Expected {
                kind: "relayed_message",
                chain_id: DESTINATION_CHAIN_ID as i64,
                block_hash: destination_block,
                block_number: 895_701,
                timestamp: 1_794_670_100,
                tx_hash: destination_tx,
                log_index: 3,
                event_data: serde_json::json!({
                    "source_chain_id": "420130019",
                    "nonce": "0",
                    "return_data_hash": "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                }),
            },
        ];

        assert_eq!(consolidated.op_interop_legs.len(), expected.len());
        for (leg, expected) in consolidated.op_interop_legs.iter().zip(expected) {
            assert_eq!(value(&leg.event_kind), expected.kind);
            assert_eq!(value(&leg.bridge_id), BRIDGE_ID);
            assert_eq!(value(&leg.chain_id), expected.chain_id);
            assert_eq!(value(&leg.block_hash), expected.block_hash);
            assert_eq!(value(&leg.log_index), expected.log_index);
            assert_eq!(value(&leg.block_number), expected.block_number);
            assert_eq!(
                value(&leg.block_timestamp).and_utc().timestamp(),
                expected.timestamp
            );
            assert_eq!(value(&leg.tx_hash), expected.tx_hash);
            assert_eq!(value(&leg.message_id), key().message_id);
            assert_eq!(value(&leg.message_hash), MESSAGE_0_HASH.to_vec());
            assert_eq!(value(&leg.event_data), expected.event_data);
            // One `insert_many` cannot mix Set and NotSet for a column.
            assert!(matches!(leg.created_at, ActiveValue::NotSet));
            assert!(matches!(leg.updated_at, ActiveValue::NotSet));
        }
    }

    #[test]
    fn test_identifier_cross_check_detects_mismatch() {
        let source = fixture_source();
        let executing = fixture_destination()
            .executing
            .expect("fixture relay is paired");

        assert!(identifier_matches_source(&executing, &source));

        type Break = fn(&mut ExecutingLeg);
        let breaks: [Break; 5] = [
            // The classic confusion: the message hash where the payload hash
            // belongs.
            |exec| exec.msg_hash = MESSAGE_0_HASH,
            |exec| exec.source_chain_id += U256::from(1),
            |exec| exec.source_block_number += U256::from(1),
            |exec| exec.source_log_index += U256::from(1),
            |exec| exec.source_timestamp += U256::from(1),
        ];
        for break_identifier in breaks {
            let mut broken = executing.clone();
            break_identifier(&mut broken);
            assert!(!identifier_matches_source(&broken, &source));
        }

        // A mismatch is a diagnostic: the outcome does not change.
        let mut destination = fixture_destination();
        if let Some(exec) = destination.executing.as_mut() {
            exec.msg_hash = MESSAGE_0_HASH;
        }
        let message = Message {
            source: Some(source),
            destination: Some(destination),
        };
        let consolidated = consolidated(&message);
        assert!(consolidated.is_final);
        assert_eq!(
            value(&consolidated.message.status),
            MessageStatus::Completed
        );
        assert_eq!(consolidated.op_interop_legs.len(), 3);
    }
}
