mod abi;
mod consolidation;
mod events;
pub mod indexer;
pub(crate) mod metrics;
pub mod settings;
mod types;
mod version;

pub use indexer::{XDaiChainConfig, XDaiContractConfig, XDaiIndexer};
pub use settings::XDaiIndexerSettings;
