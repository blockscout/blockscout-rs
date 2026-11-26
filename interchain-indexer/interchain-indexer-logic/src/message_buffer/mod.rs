// SPDX-License-Identifier: LicenseRef-Blockscout

mod buffer;
mod buffer_item;
mod cursor;
mod maintenance;
mod metrics;
mod persistence;
mod types;

pub use buffer::MessageBuffer;
pub(crate) use persistence::ProvisionalEvent;
pub use types::{
    Consolidate, ConsolidatedMessage, DestinationExecution, DetachedConfirmations, Key,
};

pub(crate) fn token_keys_from_flushed_for_enrichment(
    flushed: &[ConsolidatedMessage],
) -> Vec<(i64, Vec<u8>)> {
    persistence::token_keys_from_flushed_for_enrichment(flushed)
}

/// A writer of `op_interop_events`, shared by the maintenance flush (which uses
/// this same function) and OP interop ingestion (ADR-020). Together with
/// `retract_op_interop_events`, it is one of the table's only two writers.
pub(crate) async fn upsert_op_interop_events(
    db: &impl sea_orm::ConnectionTrait,
    legs: Vec<interchain_indexer_entity::op_interop_events::ActiveModel>,
) -> Result<(), sea_orm::DbErr> {
    persistence::upsert_op_interop_events(db, legs).await
}

// Internal re-exports for sibling submodules (maintenance, persistence).
use buffer_item::{BufferItem, BufferItemVersion};

/// Provisional ledger rows the confirmation sweep may retract (ADR-022).
pub(crate) async fn provisional_op_interop_events(
    db: &impl sea_orm::ConnectionTrait,
    bridge_id: i32,
    chain_id: i64,
    frontier: i64,
    limit: i64,
) -> Result<Vec<ProvisionalEvent>, sea_orm::DbErr> {
    persistence::provisional_op_interop_events(db, bridge_id, chain_id, frontier, limit).await
}

/// Marks provisional ledger rows `retracted` (the confirmation sweep, ADR-022).
pub(crate) async fn retract_op_interop_events(
    db: &impl sea_orm::ConnectionTrait,
    bridge_id: i32,
    chain_id: i64,
    keys: &[(Vec<u8>, i64)],
) -> Result<u64, sea_orm::DbErr> {
    persistence::retract_op_interop_events(db, bridge_id, chain_id, keys).await
}
