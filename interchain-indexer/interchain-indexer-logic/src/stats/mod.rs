//! Statistics orchestration ([`StatsService`]) and batch projection into stats tables.

mod list_query;
pub(crate) mod projection;
mod service;

pub use crate::stats_chains_query::StatsChainListRow;
pub use list_query::StatsListQuery;
pub use service::{BridgedTokenListRow, StatsService};
