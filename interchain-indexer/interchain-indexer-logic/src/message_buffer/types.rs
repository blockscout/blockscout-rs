// SPDX-License-Identifier: LicenseRef-Blockscout

use anyhow::Result;
use chrono::NaiveDateTime;
use interchain_indexer_entity::{
    amb_message_anomalies, amb_messages_confirmations, crosschain_messages, crosschain_transfers,
};
use serde::{Deserialize, Serialize};

use super::cursor::BridgeId;

/// Key for identifying a cross-chain message within a specific bridge.
///
/// The caller defines how `message_id` is derived from chain-specific fields.
/// Example: the Avalanche indexer maps Teleporter `messageID` to a compact
/// `i64` (first 8 bytes, big-endian) and combines it with `bridge_id`.
#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct Key {
    pub message_id: i64,
    pub bridge_id: BridgeId,
}

impl Key {
    pub fn new(message_id: i64, bridge_id: BridgeId) -> Self {
        Self {
            message_id,
            bridge_id,
        }
    }
}

/// Result of consolidating a working entry into final storage models.
///
/// `is_final` controls whether the entry can be removed from both tiers after
/// a successful flush.
#[derive(Clone, Debug)]
pub struct ConsolidatedMessage {
    pub is_final: bool,
    /// Delete an existing final row for this `(id, bridge_id)` before inserting
    /// this payload. Used when a protocol-specific collision intentionally
    /// displaces the old body; normal out-of-order merges must keep this false.
    pub replace_existing: bool,
    pub message: crosschain_messages::ActiveModel,
    pub transfers: Vec<crosschain_transfers::ActiveModel>,
    pub amb_confirmations: Vec<amb_messages_confirmations::ActiveModel>,
    /// AMB-specific `messageId` collision captures. Internal-only, append-only.
    /// Avalanche leaves this empty (mirrors `amb_confirmations`).
    pub amb_anomalies: Vec<amb_message_anomalies::ActiveModel>,
}

/// One observed destination-execution, in a form suitable for writing as an
/// anomaly row. Deliberately neutral: both a nonce and a hash observation can
/// turn out to be canonical or late depending on processing order, so this is
/// **not** "hash anomalies" -- see `xdai::consolidation::Message::destination_executions`
/// for the concrete producer and `message_buffer::persistence::reconcile_destination_executions`
/// for the promotion decision (made against the database, never here).
#[derive(Clone, Debug)]
pub struct DestinationExecution {
    pub key: Key,
    /// The raw destination alias exactly as observed, 32 bytes. May differ
    /// from `crosschain_messages.native_id` for the same message: the latter
    /// is a chain‖nonce blob when identity is nonce-keyed, while this is
    /// always the raw observed bytes32.
    pub native_id: Vec<u8>,
    /// The destination chain.
    pub chain_id: i64,
    pub tx_hash: Vec<u8>,
    /// Provenance only; never part of execution identity (that is the
    /// destination transaction, not the log).
    pub log_index: Option<i64>,
    pub block_number: i64,
    pub block_timestamp: NaiveDateTime,
    /// The payout recipient.
    pub executor: Option<Vec<u8>>,
    pub src_chain_id: Option<i64>,
    pub dst_chain_id: Option<i64>,
    /// Stable reason plus the kind of identity observed.
    pub detail: String,
}

/// Converts an in-flight entry into a consolidated database payload.
///
/// Returning:
/// - `Ok(None)` means the entry is *not yet consolidatable* (e.g. missing
///   the required source-side event). The buffer will keep it in hot/cold
///   storage and try again later.
/// - `Ok(Some(..))` yields the models to upsert into final tables.
///
/// The implementation decides when an entry becomes *final*.
pub trait Consolidate:
    Clone + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de>
{
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>>;

    /// Observed destination-executions, in first-appearance order.
    ///
    /// Defaulted to empty: AMB and Avalanche do not participate, and an empty
    /// channel must cost zero additional queries -- this method is called
    /// from `plan_maintenance`, **before** the maintenance transaction opens,
    /// so it returns a plain `Vec` rather than a `Result`. An `Err` there
    /// would abort plan building for the entire bridge on every cycle; see the
    /// skip site documented in `xdai::consolidation::resolve_input`
    /// (`consolidation.rs`) for the same trap applied to a different method.
    fn destination_executions(&self, _key: &Key) -> Vec<DestinationExecution> {
        Vec::new()
    }
}
