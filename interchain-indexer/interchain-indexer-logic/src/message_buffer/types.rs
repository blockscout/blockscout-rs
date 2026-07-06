// SPDX-License-Identifier: LicenseRef-Blockscout

use anyhow::Result;
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
}
