// SPDX-License-Identifier: LicenseRef-Blockscout

use std::ops::Range;

use crate::utils::interval_24h;
use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, QueryFilter,
    sea_query::{Expr, ExprTrait, SimpleExpr},
};

/// Filter for the 24 hours preceding `filter_24h_until` (both ends inclusive).
///
/// `timestamp_expr` is kept bare on the left-hand side of both comparisons so
/// that a b-tree index on the underlying column stays usable. The equivalent
/// `filter_24h_until - column at time zone 'UTC' <= interval '24 hours'` form
/// wraps the column in an expression, which is not sargable and makes postgres
/// sequential-scan the whole table.
pub fn interval_24h_filter(
    timestamp_expr: SimpleExpr,
    filter_24h_until: DateTime<Utc>,
) -> SimpleExpr {
    let interval = interval_24h(filter_24h_until);
    timestamp_expr
        .clone()
        .gte(Expr::value(*interval.start()))
        .and(timestamp_expr.lte(Expr::value(*interval.end())))
}

pub fn datetime_range_filter<Q: QueryFilter, C: ColumnTrait>(
    query: Q,
    column: C,
    range: &Range<DateTime<Utc>>,
) -> Q {
    query
        .filter(column.lt(range.end))
        .filter(column.gte(range.start))
}
