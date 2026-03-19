mod buffer;
mod buffer_item;
mod cursor;
mod maintenance;
mod metrics;
mod persistence;
mod types;

pub use buffer::MessageBuffer;
pub use types::{Consolidate, ConsolidatedMessage, Key};

pub(crate) fn token_keys_from_finalized_for_enrichment(
    finalized: &[ConsolidatedMessage],
) -> Vec<(i64, Vec<u8>)> {
    persistence::token_keys_from_finalized_for_enrichment(finalized)
}

// Internal re-exports for sibling submodules (maintenance, persistence).
use buffer_item::{BufferItem, BufferItemVersion};
