//! Service mode for DB and query branching.
//! Used by both the stats library and stats-server settings.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Single blockscout instance
    Blockscout,
    /// Multichain aggregator
    MultichainAggregator,
    /// Zetachain instance
    Zetachain,
    /// Interchain indexer (a.k.a. Universal Bridge Indexer)
    Interchain,
}
