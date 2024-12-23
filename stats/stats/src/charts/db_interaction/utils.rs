use chrono::{DateTime, Utc};
use sea_orm::sea_query::{Expr, SimpleExpr};

pub fn interval_24h_filter(
    timestamp_expr: SimpleExpr,
    filter_24h_until: DateTime<Utc>,
) -> SimpleExpr {
    Expr::cust_with_exprs(
        "$1 - $2 at time zone 'UTC' <= interval '24 hours'",
        [
            Expr::value(filter_24h_until.clone()),
            timestamp_expr.clone(),
        ],
    )
    .and(Expr::cust_with_exprs(
        "$1 - $2 at time zone 'UTC' >= interval '0 hours'",
        [Expr::value(filter_24h_until), timestamp_expr],
    ))
}
