// SPDX-License-Identifier: LicenseRef-Blockscout

mod buffer;
mod buffer_item;
mod cursor;
mod maintenance;
mod metrics;
mod persistence;
mod types;

pub use buffer::MessageBuffer;
pub use types::{Consolidate, ConsolidatedMessage, Key};

pub(crate) fn token_keys_from_flushed_for_enrichment(
    flushed: &[ConsolidatedMessage],
) -> Vec<(i64, Vec<u8>)> {
    persistence::token_keys_from_flushed_for_enrichment(flushed)
}

// Internal re-exports for sibling submodules (maintenance, persistence).
use buffer_item::{BufferItem, BufferItemVersion};
