//! Statistics orchestration ([`StatsService`]) and batch projection into stats tables.

pub(crate) mod projection;
mod service;

pub use crate::stats_chains_query::StatsChainListRow;
pub use service::{BridgedTokenListRow, StatsService};
