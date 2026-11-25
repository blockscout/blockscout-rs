// SPDX-License-Identifier: LicenseRef-Blockscout

mod buffer;
mod buffer_item;
mod cursor;
mod maintenance;
mod metrics;
mod persistence;
mod types;

pub use buffer::MessageBuffer;
pub use types::{
    Consolidate, ConsolidatedMessage, DestinationExecution, DetachedConfirmations, Key,
};

pub(crate) fn token_keys_from_flushed_for_enrichment(
    flushed: &[ConsolidatedMessage],
) -> Vec<(i64, Vec<u8>)> {
    persistence::token_keys_from_flushed_for_enrichment(flushed)
}

/// The only write of `op_interop_legs`, shared by the maintenance flush and OP
/// interop ingestion (ADR-020).
pub(crate) async fn upsert_op_interop_legs(
    db: &impl sea_orm::ConnectionTrait,
    legs: Vec<interchain_indexer_entity::op_interop_legs::ActiveModel>,
) -> Result<(), sea_orm::DbErr> {
    persistence::upsert_op_interop_legs(db, legs).await
}

// Internal re-exports for sibling submodules (maintenance, persistence).
use buffer_item::{BufferItem, BufferItemVersion};
