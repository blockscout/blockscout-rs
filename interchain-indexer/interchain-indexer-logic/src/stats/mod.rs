// SPDX-License-Identifier: LicenseRef-Blockscout

//! Statistics orchestration ([`StatsService`]) and batch projection into stats tables.

pub(crate) mod indexed_chains;
mod list_query;
pub(crate) mod metrics;
pub(crate) mod projection;
mod service;

pub use crate::stats_chains_query::StatsChainListRow;
pub use indexed_chains::IndexedChains;
pub use list_query::StatsListQuery;
pub use service::{BridgedTokenListRow, StatsReadSettings, StatsService};
