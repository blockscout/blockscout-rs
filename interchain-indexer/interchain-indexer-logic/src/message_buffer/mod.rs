mod buffer;
mod buffer_item;
mod cursor;
mod maintenance;
mod metrics;
mod persistence;
mod types;

pub use buffer::MessageBuffer;
pub use types::{Consolidate, ConsolidatedMessage, Key};

// Internal re-exports for sibling submodules (maintenance, persistence).
use buffer_item::{BufferItem, BufferItemVersion};
