//! Shared parameters for the stats list endpoints (`/stats/bridged-tokens`,
//! `/stats/chains`). Both endpoints accept the same sort + cursor + search
//! shape, parameterized by their entity-specific sort field (`S`) and
//! pagination cursor (`P`).

use crate::pagination::StatsSortOrder;

/// Bundles the sort, order, page window, cursor, and search parameters that
/// every stats list endpoint accepts. Entity-specific filters (`chain_id`,
/// `chain_ids`, ...) remain separate function arguments.
#[derive(Debug, Clone)]
pub struct StatsListQuery<'a, S, P> {
    pub sort: S,
    pub order: StatsSortOrder,
    pub page_size: usize,
    pub last_page: bool,
    pub input_pagination: Option<P>,
    pub q: Option<&'a str>,
}
