// SPDX-License-Identifier: LicenseRef-Blockscout

use alloy::{
    hex,
    primitives::{Address, B256, U256},
};
use anyhow::Result;
use interchain_indexer_entity::{
    crosschain_messages, op_interop_events, sea_orm_active_enums::MessageStatus,
};
use sea_orm::ActiveValue;
use serde::Serialize;

use crate::{
    message_buffer::{Consolidate, ConsolidatedMessage, Key},
    protocol_metadata::{
        OpInterop, OpInteropEventState, OpInteropExecutingMessage, OpInteropIdentifier,
        OpInteropLegRef, ProtocolMetadata,
    },
};

use super::{
    eth_transfer::{EthPayload, RelayEthCheck, check_relay_eth, classify, transfer_model},
    lifecycle::{self, LegView},
    metrics,
    types::{DestinationLeg, ExecutingLeg, LogRef, Message, SourceLeg},
    version::OpInteropEvent,
};

/// `consolidate` never returns `Err`: an `Err` here stalls maintenance for the
/// whole bridge on every cycle, so every conversion that can fail is done at
/// ingestion (`events::log_ref`, `events::classify_chain`).
///
/// The status is the fold over the entry's candidates
/// (`lifecycle::status`, ADR-022):
///
/// | `sources` | relays | Result |
/// | --- | --- | --- |
/// | none | any | not ready: the source chain is always configured, so its `SentMessage` can still arrive |
/// | all retracted | any | `invalidated`, flushed and kept |
/// | an active one | none active | `initiated`, flushed and kept |
/// | active, confirmed | active, confirmed | `completed`, flushed and evicted |
/// | active | active, one of them provisional | `pending_finality`, flushed and kept |
///
/// Waiting for the source before the first flush is what makes the row
/// well-formed: `src_tx_hash`, sender, payload and `init_timestamp` are all
/// source facts.
impl Consolidate for Message {
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>> {
        let source_views: Vec<LegView> = self.sources.iter().map(LegView::of_source).collect();
        let relay_views: Vec<LegView> = self
            .destinations
            .iter()
            .map(LegView::of_destination)
            .collect();
        let Some(status) = lifecycle::status(&source_views, &relay_views) else {
            return Ok(None);
        };
        let Some(source) = lifecycle::shown(&source_views).map(|index| &self.sources[index]) else {
            return Ok(None);
        };
        let destination = lifecycle::shown(&relay_views).map(|index| &self.destinations[index]);
        let active_destination =
            lifecycle::active(&relay_views).map(|index| &self.destinations[index]);
        let is_final = status == MessageStatus::Completed;

        let executing =
            destination.and_then(|destination| delivering_execution(destination, source));

        let rows = StorageRows::build(source, executing);
        let legs = rows.as_ref().and_then(|rows| {
            rows.leg_models(
                key,
                &self.sources,
                &self.destinations,
                destination,
                executing,
            )
        });
        let (Some(rows), Some(legs)) = (rows, legs) else {
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

        let payload = classify(source.sender, source.target, &source.message);

        for kind in completion_anomalies(is_final, source, destination, executing, &payload) {
            let dst_tx_hash = destination.map(|destination| destination.log.transaction_hash);
            match kind {
                "identifier_mismatch" => {
                    metrics::OP_INTEROP_LEG_CONFLICTS_TOTAL
                        .with_label_values(&[key.bridge_id.to_string().as_str(), kind])
                        .inc();
                    tracing::warn!(
                        bridge_id = key.bridge_id,
                        message_id = key.message_id,
                        message_hash = %source.message_hash,
                        src_tx_hash = %source.log.transaction_hash,
                        dst_tx_hash = ?dst_tx_hash,
                        candidates = destination.map_or(0, |destination| destination.candidates().count()),
                        "OP interop relay has ExecutingMessage candidates but none points at the \
                         indexed SentMessage log; the message completes without its \
                         executing_message detail"
                    );
                }
                _ => {
                    metrics::OP_INTEROP_ETH_TRANSFER_ANOMALIES_TOTAL
                        .with_label_values(&[key.bridge_id.to_string().as_str(), kind])
                        .inc();
                    tracing::warn!(
                        bridge_id = key.bridge_id,
                        message_id = key.message_id,
                        message_hash = %source.message_hash,
                        src_tx_hash = %source.log.transaction_hash,
                        dst_tx_hash = ?dst_tx_hash,
                        kind,
                        "OP interop RelayETH event is missing or disagrees with the ETH transfer \
                         payload; the message completes anyway"
                    );
                }
            }
        }

        // Derived from the payload alone, at every status; the bridge events
        // never change it.
        let transfers = match &payload {
            EthPayload::Transfer(transfer) => transfer_model(
                key,
                rows.source.chain_id,
                rows.destination_chain_id,
                transfer,
            )
            .into_iter()
            .collect(),
            EthPayload::NotBridgeMessage | EthPayload::Unrecognized => Vec::new(),
        };

        let message = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            status: ActiveValue::Set(status),
            init_timestamp: ActiveValue::Set(source.log.block_timestamp),
            // The relay columns follow the *active* relay only: a retracted
            // relay leaves them empty. `dst_chain_id` and the recipient are
            // source facts (`SentMessage.destination`, `target`).
            last_update_timestamp: ActiveValue::Set(
                active_destination.map(|destination| destination.log.block_timestamp),
            ),
            src_chain_id: ActiveValue::Set(rows.source.chain_id),
            dst_chain_id: ActiveValue::Set(Some(rows.destination_chain_id)),
            native_id: ActiveValue::Set(Some(source.message_hash.to_vec())),
            src_tx_hash: ActiveValue::Set(Some(source.log.transaction_hash.to_vec())),
            dst_tx_hash: ActiveValue::Set(
                active_destination.map(|destination| destination.log.transaction_hash.to_vec()),
            ),
            // The `sendMessage` caller (`msg.sender` in the messenger), not
            // the transaction origin: no transaction is fetched. It is a
            // contract for most traffic.
            sender_address: ActiveValue::Set(Some(source.sender.to_vec())),
            recipient_address: ActiveValue::Set(Some(source.target.to_vec())),
            payload: ActiveValue::Set(Some(source.message.to_vec())),
            protocol_metadata: ActiveValue::Set(
                ProtocolMetadata {
                    op_interop: Some(metadata(source, destination, executing)),
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
            transfers,
            amb_confirmations: Vec::new(),
            amb_anomalies: Vec::new(),
            op_interop_events: legs,
            unsettled: self.is_unsettled(),
        }))
    }
}

/// The consolidation-time anomalies to report: only for the consolidation
/// that completes the message, so provisional re-flushes do not count the
/// same pair again. Kinds: `identifier_mismatch` (`OP_INTEROP_LEG_CONFLICTS_TOTAL`),
/// `relay_eth_missing` / `relay_eth_mismatch`
/// (`OP_INTEROP_ETH_TRANSFER_ANOMALIES_TOTAL`).
fn completion_anomalies(
    is_final: bool,
    source: &SourceLeg,
    destination: Option<&DestinationLeg>,
    executing: Option<&ExecutingLeg>,
    payload: &EthPayload,
) -> Vec<&'static str> {
    let Some(destination) = destination.filter(|_| is_final) else {
        return Vec::new();
    };
    let mut kinds = Vec::new();
    if executing.is_none() && destination.candidates().next().is_some() {
        kinds.push("identifier_mismatch");
    }
    match check_relay_eth(payload, source.log.chain_id, destination.relay_eth.as_ref()) {
        RelayEthCheck::Ok => {}
        RelayEthCheck::Missing => kinds.push("relay_eth_missing"),
        RelayEthCheck::Mismatch => kinds.push("relay_eth_mismatch"),
    }
    kinds
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
    executing: Option<StoredLogRef>,
}

impl StorageRows {
    fn build(source: &SourceLeg, executing: Option<&ExecutingLeg>) -> Option<Self> {
        Some(Self {
            source: StoredLogRef::new(&source.log)?,
            destination_chain_id: i64::try_from(source.destination_chain_id).ok()?,
            executing: match executing {
                Some(executing) => Some(StoredLogRef::new(&executing.log)?),
                None => None,
            },
        })
    }

    /// One ledger row per log the entry holds: a `sent_message` per source
    /// candidate, the delivering `executing_message` of the shown pair (its
    /// state is the shown relay's), and a `relayed_message` per destination
    /// candidate. The other execution candidates are raw executions that
    /// ingestion already wrote. `None` if any of them does not fit its storage
    /// type.
    ///
    /// Every model sets the same columns and leaves `created_at` /
    /// `updated_at` unset: SeaORM cannot mix `Set` and `NotSet` for one column
    /// in one `insert_many`.
    fn leg_models(
        &self,
        key: &Key,
        sources: &[SourceLeg],
        destinations: &[DestinationLeg],
        shown_destination: Option<&DestinationLeg>,
        executing: Option<&ExecutingLeg>,
    ) -> Option<Vec<op_interop_events::ActiveModel>> {
        let bridge_id = key.bridge_id as i32;
        let mut models = sources
            .iter()
            .map(|source| sent_row(bridge_id, key.message_id, source, source.state))
            .collect::<Option<Vec<_>>>()?;

        if let (Some(destination), Some(executing), Some(stored)) =
            (shown_destination, executing, &self.executing)
        {
            models.push(executing_model(
                bridge_id,
                executing,
                stored,
                Some((key.message_id, destination.message_hash)),
                destination.state,
            ));
        }
        for destination in destinations {
            models.push(relayed_row(
                bridge_id,
                key.message_id,
                destination,
                destination.state,
            )?);
        }
        Some(models)
    }
}

/// A link from a ledger row to its message: `(message_id, message_hash)`.
type LegLink = (i64, B256);

/// `src_*` columns: the `Identifier` join key in storage types. A value that
/// does not fit `i64` is `None` (stored `NULL`); `event_data` keeps it
/// verbatim. Never a failure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StoredIdentifier {
    chain_id: Option<i64>,
    block_number: Option<i64>,
    log_index: Option<i64>,
}

impl StoredIdentifier {
    fn new(executing: &ExecutingLeg) -> Self {
        Self {
            chain_id: i64::try_from(executing.source_chain_id).ok(),
            block_number: i64::try_from(executing.source_block_number).ok(),
            log_index: i64::try_from(executing.source_log_index).ok(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn leg_model(
    bridge_id: i32,
    link: Option<LegLink>,
    log: &LogRef,
    stored: &StoredLogRef,
    event: OpInteropEvent,
    event_data: serde_json::Value,
    identifier: StoredIdentifier,
    state: OpInteropEventState,
) -> op_interop_events::ActiveModel {
    op_interop_events::ActiveModel {
        bridge_id: ActiveValue::Set(bridge_id),
        chain_id: ActiveValue::Set(stored.chain_id),
        block_hash: ActiveValue::Set(log.block_hash.to_vec()),
        log_index: ActiveValue::Set(stored.log_index),
        block_number: ActiveValue::Set(stored.block_number),
        block_timestamp: ActiveValue::Set(log.block_timestamp),
        tx_hash: ActiveValue::Set(log.transaction_hash.to_vec()),
        event_kind: ActiveValue::Set(event.as_str().to_string()),
        message_id: ActiveValue::Set(link.map(|(message_id, _)| message_id)),
        message_hash: ActiveValue::Set(link.map(|(_, message_hash)| message_hash.to_vec())),
        src_chain_id: ActiveValue::Set(identifier.chain_id),
        src_block_number: ActiveValue::Set(identifier.block_number),
        src_log_index: ActiveValue::Set(identifier.log_index),
        event_data: ActiveValue::Set(event_data),
        state: ActiveValue::Set(state.as_str().to_string()),
        confirmed_seen_at: ActiveValue::NotSet,
        retracted_seen_at: ActiveValue::NotSet,
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    }
}

/// The `executing_message` row, identical for both writers but for `link`.
fn executing_model(
    bridge_id: i32,
    executing: &ExecutingLeg,
    stored: &StoredLogRef,
    link: Option<LegLink>,
    state: OpInteropEventState,
) -> op_interop_events::ActiveModel {
    leg_model(
        bridge_id,
        link,
        &executing.log,
        stored,
        OpInteropEvent::ExecutingMessage,
        ExecutingMessageData::new(executing).to_json(),
        StoredIdentifier::new(executing),
        state,
    )
}

/// The ledger row of one `ExecutingMessage` for ingestion. `None` only if the
/// execution's own location does not fit the storage types, which
/// `events::log_ref` already rules out.
pub(super) fn execution_row(
    bridge_id: i32,
    executing: &ExecutingLeg,
    link: Option<LegLink>,
    state: OpInteropEventState,
) -> Option<op_interop_events::ActiveModel> {
    StoredLogRef::new(&executing.log)
        .map(|stored| executing_model(bridge_id, executing, &stored, link, state))
}

/// The `sent_message` row of one source leg, linked to its message. The
/// flush builds the identical row (ADR-020: every leg is written when read).
/// `None` only if the log's location does not fit the storage types, which
/// `events::log_ref` already rules out.
pub(super) fn sent_row(
    bridge_id: i32,
    message_id: i64,
    source: &SourceLeg,
    state: OpInteropEventState,
) -> Option<op_interop_events::ActiveModel> {
    StoredLogRef::new(&source.log).map(|stored| {
        leg_model(
            bridge_id,
            Some((message_id, source.message_hash)),
            &source.log,
            &stored,
            OpInteropEvent::SentMessage,
            SentMessageData::new(source).to_json(),
            StoredIdentifier::default(),
            state,
        )
    })
}

/// The `relayed_message` row of one destination leg, linked to its message.
pub(super) fn relayed_row(
    bridge_id: i32,
    message_id: i64,
    destination: &DestinationLeg,
    state: OpInteropEventState,
) -> Option<op_interop_events::ActiveModel> {
    StoredLogRef::new(&destination.log).map(|stored| {
        leg_model(
            bridge_id,
            Some((message_id, destination.message_hash)),
            &destination.log,
            &stored,
            OpInteropEvent::RelayedMessage,
            RelayedMessageData::new(destination).to_json(),
            StoredIdentifier::default(),
            state,
        )
    })
}

/// The execution that delivered `source`'s message: among the relay's
/// candidates, the one whose identifier and payload hash point at the held
/// `SentMessage` log; the lowest log index if several do (two reads of the
/// same log are indistinguishable). `None`: nothing is nested or linked.
pub(super) fn delivering_execution<'a>(
    destination: &'a DestinationLeg,
    source: &SourceLeg,
) -> Option<&'a ExecutingLeg> {
    destination
        .candidates()
        .filter(|executing| identifier_matches_source(executing, source))
        .min_by_key(|executing| executing.log.log_index)
}

/// Lower-case `0x` hex, the form every hash and address takes in
/// `event_data` and in `protocol_metadata`.
fn hex_hash(value: B256) -> String {
    hex::encode_prefixed(value)
}

fn hex_address(value: Address) -> String {
    hex::encode_prefixed(value)
}

/// `op_interop_events.event_data` of a `sent_message` row. The message bytes
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

/// `op_interop_events.event_data` of a `relayed_message` row.
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

/// `op_interop_events.event_data` of an `executing_message` row.
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

fn leg_ref(log: &LogRef, safety: OpInteropEventState, retracted_tx_hash: bool) -> OpInteropLegRef {
    OpInteropLegRef {
        block_number: log.block_number,
        block_hash: hex_hash(log.block_hash),
        log_index: log.log_index,
        safety,
        tx_hash: retracted_tx_hash.then(|| hex_hash(log.transaction_hash)),
    }
}

/// The `op_interop` namespace of `protocol_metadata`: `nonce` and `source`
/// from the first flush on, `destination` once the relay is indexed (a
/// retracted one stays, with its `tx_hash`), and `executing_message` only when
/// a candidate delivered the message. Legs carry their `safety`.
fn metadata(
    source: &SourceLeg,
    destination: Option<&DestinationLeg>,
    executing: Option<&ExecutingLeg>,
) -> OpInterop {
    OpInterop {
        nonce: source.nonce.to_string(),
        source: Some(leg_ref(&source.log, source.state, false)),
        destination: destination.map(|destination| {
            leg_ref(
                &destination.log,
                destination.state,
                destination.state == OpInteropEventState::Retracted,
            )
        }),
        executing_message: executing.map(|executing| OpInteropExecutingMessage {
            msg_hash: hex_hash(executing.msg_hash),
            log_index: executing.log.log_index,
            identifier: identifier(executing),
        }),
    }
}

/// Does this `ExecutingMessage` point at the `SentMessage` log this indexer
/// holds for the message?
///
/// `msgHash` must be the payload hash of that log, and the identifier its
/// `(chainId, blockNumber, logIndex, timestamp)`. It is the content test that
/// selects the delivering execution among a relay's candidates.
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
            .op_interop_events
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
            sources: vec![],
            destinations: vec![fixture_destination()],
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
            sources: vec![source.clone()],
            destinations: vec![],
        };

        let consolidated = consolidated(&message);

        assert!(!consolidated.is_final);
        assert!(!consolidated.replace_existing);
        assert_eq!(
            consolidated.transfers.len(),
            1,
            "the ETH transfer, from the first flush"
        );
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
                        "log_index": 1,
                        "safety": "confirmed"
                    }
                }
            })
        );
    }

    fn assert_fixture_transfer(consolidated: &ConsolidatedMessage) {
        assert_eq!(consolidated.transfers.len(), 1);
        let transfer = &consolidated.transfers[0];
        let amount: sea_orm::prelude::BigDecimal = "1000000000000000000".parse().unwrap();
        assert_eq!(value(&transfer.message_id), key().message_id);
        assert_eq!(value(&transfer.bridge_id), BRIDGE_ID);
        assert_eq!(value(&transfer.index), 0);
        assert_eq!(value(&transfer.token_src_chain_id), SOURCE_CHAIN_ID as i64);
        assert_eq!(
            value(&transfer.token_dst_chain_id),
            DESTINATION_CHAIN_ID as i64
        );
        assert_eq!(value(&transfer.token_src_address), Some(vec![0; 20]));
        assert_eq!(value(&transfer.token_dst_address), Some(vec![0; 20]));
        assert_eq!(value(&transfer.src_amount), Some(amount.clone()));
        assert_eq!(value(&transfer.dst_amount), Some(amount));
        assert_eq!(
            value(&transfer.sender_address),
            Some(Address::repeat_byte(0x11).to_vec())
        );
        assert_eq!(
            value(&transfer.recipient_address),
            Some(Address::repeat_byte(0x22).to_vec())
        );
    }

    #[test]
    fn test_consolidate_eth_source_only_has_one_transfer_and_keeps_bridge_addresses() {
        let source = fixture_source();
        let consolidated = consolidated(&Message {
            sources: vec![source.clone()],
            destinations: vec![],
        });
        assert_fixture_transfer(&consolidated);
        assert_eq!(
            value(&consolidated.message.sender_address),
            Some(source.sender.to_vec()),
            "the message row keeps the bridge address"
        );
    }

    #[test]
    fn test_consolidate_eth_both_legs_has_the_same_transfer() {
        let consolidated = consolidated(&Message {
            sources: vec![fixture_source()],
            destinations: vec![fixture_destination()],
        });
        assert!(consolidated.is_final);
        assert_fixture_transfer(&consolidated);
    }

    #[test]
    fn test_consolidate_eth_without_relay_eth_still_completes_with_the_transfer() {
        let mut destination = fixture_destination();
        destination.relay_eth = None;
        let consolidated = consolidated(&Message {
            sources: vec![fixture_source()],
            destinations: vec![destination],
        });
        assert_eq!(
            value(&consolidated.message.status),
            MessageStatus::Completed
        );
        assert_fixture_transfer(&consolidated);
    }

    #[test]
    fn test_consolidate_non_bridge_message_has_no_transfer() {
        let mut source = fixture_source();
        source.sender = Address::repeat_byte(0x11);
        let consolidated = consolidated(&Message {
            sources: vec![source],
            destinations: vec![],
        });
        assert!(consolidated.transfers.is_empty());
    }

    #[test]
    fn test_consolidate_unrecognized_bridge_payload_has_no_transfer_but_consolidates() {
        let mut source = fixture_source();
        let mut message = source.message.to_vec();
        message[0] ^= 0xff;
        source.message = message.into();
        let consolidated = consolidated(&Message {
            sources: vec![source],
            destinations: vec![fixture_destination()],
        });
        assert!(consolidated.transfers.is_empty());
        assert_eq!(
            value(&consolidated.message.status),
            MessageStatus::Completed
        );
    }

    #[test]
    fn test_consolidate_both_legs_is_completed_and_final() {
        let source = fixture_source();
        let destination = fixture_destination();
        let message = Message {
            sources: vec![source.clone()],
            destinations: vec![destination.clone()],
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
        assert_eq!(consolidated.op_interop_events.len(), 3);
    }

    #[test]
    fn test_consolidate_unpaired_relay_has_two_leg_rows_and_no_executing_metadata() {
        let mut destination = fixture_destination();
        destination.executing_candidates.clear();
        let message = Message {
            sources: vec![fixture_source()],
            destinations: vec![destination],
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
            sources: vec![fixture_source()],
            destinations: vec![fixture_destination()],
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
                        "log_index": 1,
                        "safety": "confirmed"
                    },
                    "destination": {
                        "block_number": 895701,
                        "block_hash": "0x7ea9e26557b70150ae7360da608f0f231e30397b4e5b4c98f4edd431d6630c73",
                        "log_index": 3,
                        "safety": "confirmed"
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
            src: (Option<i64>, Option<i64>, Option<i64>),
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
                src: (None, None, None),
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
                src: (Some(420_130_019), Some(498_922), Some(1)),
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
                src: (None, None, None),
                event_data: serde_json::json!({
                    "source_chain_id": "420130019",
                    "nonce": "0",
                    "return_data_hash": "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                }),
            },
        ];

        assert_eq!(consolidated.op_interop_events.len(), expected.len());
        for (leg, expected) in consolidated.op_interop_events.iter().zip(expected) {
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
            assert_eq!(value(&leg.message_id), Some(key().message_id));
            assert_eq!(value(&leg.message_hash), Some(MESSAGE_0_HASH.to_vec()));
            assert_eq!(
                (
                    value(&leg.src_chain_id),
                    value(&leg.src_block_number),
                    value(&leg.src_log_index)
                ),
                expected.src
            );
            assert_eq!(value(&leg.event_data), expected.event_data);
            // One `insert_many` cannot mix Set and NotSet for a column.
            assert!(matches!(leg.created_at, ActiveValue::NotSet));
            assert!(matches!(leg.updated_at, ActiveValue::NotSet));
        }
    }

    fn own() -> ExecutingLeg {
        fixture_destination().executing_candidates[0].clone()
    }

    /// Another read of a log of the same source block: never matches message 0.
    fn pull(log_index: u64) -> ExecutingLeg {
        let mut exec = own();
        exec.log.log_index = log_index;
        exec.source_log_index = U256::from(7);
        exec.msg_hash = B256::repeat_byte(0xA0);
        exec
    }

    fn at(mut exec: ExecutingLeg, log_index: u64) -> ExecutingLeg {
        exec.log.log_index = log_index;
        exec
    }

    fn with_candidates(candidates: Vec<ExecutingLeg>) -> DestinationLeg {
        let mut destination = fixture_destination();
        destination.executing = None;
        destination.executing_candidates = candidates;
        destination
    }

    fn complete_with(destination: DestinationLeg) -> ConsolidatedMessage {
        consolidated(&Message {
            sources: vec![fixture_source()],
            destinations: vec![destination],
        })
    }

    #[test]
    fn test_identifier_cross_check_detects_mismatch() {
        let source = fixture_source();
        let executing = own();

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
    }

    #[test]
    fn test_consolidate_links_the_candidate_that_points_at_the_source() {
        for (candidates, own_index) in [
            (vec![pull(0), at(own(), 1)], 1),
            (vec![at(own(), 0), pull(1)], 0),
        ] {
            let consolidated = complete_with(with_candidates(candidates));

            assert_eq!(
                event_kinds(&consolidated),
                vec!["sent_message", "executing_message", "relayed_message"]
            );
            let executing = &consolidated.op_interop_events[1];
            assert_eq!(value(&executing.log_index), own_index);
            assert_eq!(value(&executing.message_id), Some(key().message_id));
            let metadata = value(&consolidated.message.protocol_metadata).expect("metadata");
            assert_eq!(
                metadata["op_interop"]["executing_message"]["log_index"],
                serde_json::json!(own_index)
            );
        }
    }

    #[test]
    fn test_consolidate_without_a_matching_candidate_nests_and_links_nothing() {
        let consolidated = complete_with(with_candidates(vec![pull(0)]));

        assert!(consolidated.is_final);
        assert_eq!(
            value(&consolidated.message.status),
            MessageStatus::Completed
        );
        assert_eq!(
            event_kinds(&consolidated),
            vec!["sent_message", "relayed_message"]
        );
        let metadata = value(&consolidated.message.protocol_metadata).expect("metadata");
        assert!(metadata["op_interop"].get("executing_message").is_none());
    }

    #[test]
    fn test_consolidate_legacy_executing_is_a_candidate() {
        let mut destination = with_candidates(Vec::new());
        destination.executing = Some(own());
        let consolidated = complete_with(destination);
        assert_eq!(
            event_kinds(&consolidated),
            vec!["sent_message", "executing_message", "relayed_message"]
        );

        let mut destination = with_candidates(Vec::new());
        destination.executing = Some(pull(0));
        let consolidated = complete_with(destination);
        assert_eq!(
            event_kinds(&consolidated),
            vec!["sent_message", "relayed_message"]
        );
    }

    #[test]
    fn test_consolidate_two_matching_candidates_links_the_lower_log_index() {
        let consolidated = complete_with(with_candidates(vec![at(own(), 2), at(own(), 1)]));
        assert_eq!(value(&consolidated.op_interop_events[1].log_index), 1);
    }

    #[test]
    fn test_execution_row_columns() {
        let link = Some((key().message_id, MESSAGE_0_HASH));
        let confirmed = OpInteropEventState::Confirmed;
        let unlinked = execution_row(BRIDGE_ID, &own(), None, confirmed).expect("fits");
        let linked = execution_row(BRIDGE_ID, &own(), link, confirmed).expect("fits");

        assert_eq!(unlinked.state, Set("confirmed".to_string()));
        assert_eq!(unlinked.confirmed_seen_at, ActiveValue::NotSet);
        assert_eq!(unlinked.retracted_seen_at, ActiveValue::NotSet);

        assert_eq!(value(&unlinked.message_id), None);
        assert_eq!(value(&unlinked.message_hash), None);
        assert_eq!(value(&linked.message_id), Some(key().message_id));
        assert_eq!(value(&linked.message_hash), Some(MESSAGE_0_HASH.to_vec()));

        let mut relinked = unlinked.clone();
        relinked.message_id = linked.message_id.clone();
        relinked.message_hash = linked.message_hash.clone();
        assert_eq!(relinked, linked, "only the link differs");

        assert_eq!(
            (
                unlinked.src_chain_id.clone(),
                unlinked.src_block_number.clone(),
                unlinked.src_log_index.clone()
            ),
            (Set(Some(420_130_019)), Set(Some(498_922)), Set(Some(1)))
        );

        let mut wide = own();
        wide.source_chain_id = U256::from(1) << 64;
        wide.source_log_index = U256::from(1) << 70;
        let row = execution_row(BRIDGE_ID, &wide, None, confirmed).expect("fits");
        assert_eq!(row.src_chain_id, Set(None));
        assert_eq!(row.src_log_index, Set(None));
        assert_eq!(row.src_block_number, Set(Some(498_922)));
        let data = value(&row.event_data);
        assert_eq!(data["identifier"]["chain_id"], "18446744073709551616");
        assert_eq!(data["identifier"]["log_index"], "1180591620717411303424");

        // The boundary: i64::MAX fits, the next value (still a u64) does not.
        let mut edge = own();
        edge.source_block_number = U256::from(i64::MAX as u64);
        edge.source_log_index = U256::from(i64::MAX as u64) + U256::from(1);
        let row = execution_row(BRIDGE_ID, &edge, None, confirmed).expect("fits");
        assert_eq!(row.src_block_number, Set(Some(i64::MAX)));
        assert_eq!(row.src_log_index, Set(None));
    }

    #[test]
    fn test_sent_and_relayed_rows_match_the_flush() {
        let source = fixture_source();
        let destination = fixture_destination();
        let consolidated = consolidated(&Message {
            sources: vec![source.clone()],
            destinations: vec![destination.clone()],
        });
        let message_id = key().message_id;
        let confirmed = OpInteropEventState::Confirmed;

        let find = |kind: &str| {
            consolidated
                .op_interop_events
                .iter()
                .find(|leg| value(&leg.event_kind) == kind)
                .unwrap_or_else(|| panic!("no {kind} row"))
        };
        assert_eq!(
            find("sent_message"),
            &sent_row(BRIDGE_ID, message_id, &source, confirmed).expect("fits")
        );
        assert_eq!(
            find("relayed_message"),
            &relayed_row(BRIDGE_ID, message_id, &destination, confirmed).expect("fits")
        );
    }

    // --- Overlay states ---

    const P: OpInteropEventState = OpInteropEventState::Provisional;
    const C: OpInteropEventState = OpInteropEventState::Confirmed;
    const R: OpInteropEventState = OpInteropEventState::Retracted;

    fn source_in(state: OpInteropEventState, hash_byte: u8, block: u64, secs: i64) -> SourceLeg {
        let mut leg = fixture_source();
        leg.state = state;
        leg.log.block_hash = B256::repeat_byte(hash_byte);
        leg.log.block_number = block;
        leg.log.block_timestamp = chrono::DateTime::from_timestamp(secs, 0)
            .unwrap()
            .naive_utc();
        leg
    }

    fn relay_in(state: OpInteropEventState, hash_byte: u8) -> DestinationLeg {
        let mut leg = fixture_destination();
        leg.state = state;
        leg.log.block_hash = B256::repeat_byte(hash_byte);
        leg
    }

    fn fixture_source_in(state: OpInteropEventState) -> SourceLeg {
        SourceLeg {
            state,
            ..fixture_source()
        }
    }

    fn ledger_states(consolidated: &ConsolidatedMessage) -> Vec<(String, String)> {
        consolidated
            .op_interop_events
            .iter()
            .map(|leg| (value(&leg.event_kind), value(&leg.state)))
            .collect()
    }

    struct Expect {
        status: MessageStatus,
        is_final: bool,
        unsettled: bool,
    }

    fn check(message: &Message, expect: Expect) -> ConsolidatedMessage {
        let consolidated = consolidated(message);
        assert_eq!(value(&consolidated.message.status), expect.status);
        assert_eq!(consolidated.is_final, expect.is_final);
        assert_eq!(consolidated.unsettled, expect.unsettled);
        consolidated
    }

    fn op_interop_of(consolidated: &ConsolidatedMessage) -> serde_json::Value {
        value(&consolidated.message.protocol_metadata).expect("metadata")["op_interop"].clone()
    }

    #[test]
    fn test_consolidate_source_provisional_only_is_initiated_and_unsettled() {
        let consolidated = check(
            &Message {
                sources: vec![fixture_source_in(P)],
                destinations: vec![],
            },
            Expect {
                status: MessageStatus::Initiated,
                is_final: false,
                unsettled: true,
            },
        );
        assert_eq!(
            op_interop_of(&consolidated)["source"]["safety"],
            "provisional"
        );
        assert_eq!(
            ledger_states(&consolidated),
            vec![("sent_message".to_string(), "provisional".to_string())]
        );
    }

    #[test]
    fn test_consolidate_relay_provisional_is_pending_finality() {
        let consolidated = check(
            &Message {
                sources: vec![fixture_source()],
                destinations: vec![relay_in(P, 0x7E)],
            },
            Expect {
                status: MessageStatus::PendingFinality,
                is_final: false,
                unsettled: true,
            },
        );
        let row = &consolidated.message;
        assert!(value(&row.dst_tx_hash).is_some());
        assert!(value(&row.last_update_timestamp).is_some());
        assert_eq!(
            op_interop_of(&consolidated)["destination"]["safety"],
            "provisional"
        );
        let states: Vec<String> = ledger_states(&consolidated)
            .into_iter()
            .map(|(_, state)| state)
            .collect();
        assert_eq!(states, vec!["confirmed", "provisional", "provisional"]);
    }

    #[test]
    fn test_consolidate_both_confirmed_is_completed_and_settled() {
        check(
            &Message {
                sources: vec![fixture_source()],
                destinations: vec![fixture_destination()],
            },
            Expect {
                status: MessageStatus::Completed,
                is_final: true,
                unsettled: false,
            },
        );
    }

    #[test]
    fn test_consolidate_only_source_retracted_is_invalidated() {
        let consolidated = check(
            &Message {
                sources: vec![fixture_source_in(R)],
                destinations: vec![],
            },
            Expect {
                status: MessageStatus::Invalidated,
                is_final: false,
                unsettled: true,
            },
        );
        assert_eq!(
            op_interop_of(&consolidated)["source"]["safety"],
            "retracted"
        );
    }

    #[test]
    fn test_consolidate_relay_retracted_returns_to_initiated() {
        let retracted = relay_in(R, 0x7E);
        let tx_hash = hex_hash(retracted.log.transaction_hash);
        let consolidated = check(
            &Message {
                sources: vec![fixture_source()],
                destinations: vec![retracted],
            },
            Expect {
                status: MessageStatus::Initiated,
                is_final: false,
                unsettled: true,
            },
        );
        let row = &consolidated.message;
        assert_eq!(value(&row.dst_tx_hash), None);
        assert_eq!(value(&row.last_update_timestamp), None);
        let op_interop = op_interop_of(&consolidated);
        assert_eq!(op_interop["destination"]["safety"], "retracted");
        assert_eq!(op_interop["destination"]["tx_hash"], tx_hash);
        assert!(
            op_interop["source"].get("tx_hash").is_none(),
            "only a retracted destination carries its tx_hash"
        );
        assert!(
            op_interop.get("executing_message").is_some(),
            "the retracted relay's execution stays"
        );
    }

    #[test]
    fn test_consolidate_revival_shows_the_later_confirmed_source() {
        let first = source_in(R, 0xB1, 498_922, 1_794_670_098);
        let revived = source_in(C, 0xC1, 498_930, 1_794_670_114);
        let consolidated = check(
            &Message {
                sources: vec![first, revived.clone()],
                destinations: vec![],
            },
            Expect {
                status: MessageStatus::Initiated,
                is_final: false,
                unsettled: true,
            },
        );
        assert_eq!(
            value(&consolidated.message.init_timestamp),
            revived.log.block_timestamp
        );
        let op_interop = op_interop_of(&consolidated);
        assert_eq!(op_interop["source"]["block_number"], 498_930);
        assert_eq!(op_interop["source"]["safety"], "confirmed");
        let states = ledger_states(&consolidated);
        assert_eq!(
            states,
            vec![
                ("sent_message".to_string(), "retracted".to_string()),
                ("sent_message".to_string(), "confirmed".to_string())
            ]
        );
    }

    #[test]
    fn test_consolidate_two_relays_confirmed_one_wins_but_stays_unsettled() {
        let consolidated = check(
            &Message {
                sources: vec![fixture_source()],
                destinations: vec![relay_in(P, 0xA1), fixture_destination()],
            },
            Expect {
                status: MessageStatus::Completed,
                is_final: true,
                unsettled: true,
            },
        );
        let op_interop = op_interop_of(&consolidated);
        assert_eq!(op_interop["destination"]["safety"], "confirmed");
        assert_eq!(
            op_interop["destination"]["block_hash"],
            hex_hash(fixture_destination().log.block_hash)
        );
    }

    #[test]
    fn test_completion_anomalies_only_on_the_completing_consolidation() {
        let source = fixture_source();
        let mut destination = fixture_destination();
        // No candidate points at the source log.
        for candidate in &mut destination.executing_candidates {
            candidate.source_log_index += U256::from(1);
        }
        let executing = delivering_execution(&destination, &source);
        assert!(executing.is_none());
        let payload = classify(source.sender, source.target, &source.message);

        let kinds = completion_anomalies(true, &source, Some(&destination), executing, &payload);
        assert_eq!(kinds, vec!["identifier_mismatch"]);
        assert!(
            completion_anomalies(false, &source, Some(&destination), executing, &payload)
                .is_empty()
        );
        assert!(completion_anomalies(true, &source, None, None, &payload).is_empty());
    }
}
