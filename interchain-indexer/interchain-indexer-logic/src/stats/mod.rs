// SPDX-License-Identifier: LicenseRef-Blockscout

//! Statistics orchestration ([`StatsService`]) and batch projection into stats tables.

pub(crate) mod indexed_chains;
mod list_query;
pub(crate) mod metrics;
pub mod overlap_warning;
pub(crate) mod projection;
mod service;

pub use crate::stats_chains_query::{StatsChainListRow, StatsChainsScope};
pub use indexed_chains::IndexedChains;
pub use list_query::StatsListQuery;
pub use overlap_warning::{OverlapTransition, overlap_transition};
pub use service::{BridgedTokenListRow, StatsReadSettings, StatsService};
