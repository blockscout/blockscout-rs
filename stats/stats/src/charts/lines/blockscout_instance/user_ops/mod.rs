//! Stats for user-ops-indexer.
//! In other words, about account abstraction as per ERC 4337.

pub mod aa_wallets_growth;
pub mod active_aa_wallets;
pub mod active_bundlers;
pub mod active_paymasters;
pub mod eip_7702_auths_growth;
pub mod new_aa_wallets;
pub mod new_eip_7702_auths;
pub mod new_user_ops;
pub mod user_ops_growth;

use std::ops::Range;

use blockscout_db::entity::{blocks, user_operations};
use chrono::{DateTime, Utc};
use migration::{Alias, Expr, Func, IntoIden, SimpleExpr};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoIdentity, IntoSimpleExpr, Order, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Statement,
};

use crate::charts::db_interaction::utils::datetime_range_filter;

pub(crate) fn count_distinct_in_user_ops(
    distinct: impl Into<SimpleExpr>,
    range: Option<Range<DateTime<Utc>>>,
) -> Statement {
    let date_intermediate_col = "date".into_identity();
    let mut query = user_operations::Entity::find()
        .select_only()
        .join(
            sea_orm::JoinType::InnerJoin,
            user_operations::Entity::belongs_to(blocks::Entity)
                .from(user_operations::Column::BlockHash)
                .to(blocks::Column::Hash)
                .into(),
        )
        .expr_as(
            blocks::Column::Timestamp
                .into_simple_expr()
                .cast_as(Alias::new("date")),
            date_intermediate_col.clone(),
        )
        .expr_as(
            SimpleExpr::from(Func::count_distinct(distinct)).cast_as(Alias::new("text")),
            "value",
        )
        .filter(blocks::Column::Consensus.eq(true))
        .filter(blocks::Column::Timestamp.ne(DateTime::UNIX_EPOCH))
        .group_by(Expr::col(date_intermediate_col.clone().into_iden()))
        .order_by(Expr::col(date_intermediate_col.into_iden()), Order::Asc);
    if let Some(range) = range {
        query = datetime_range_filter(query, blocks::Column::Timestamp, &range);
    }
    query.build(sea_orm::DatabaseBackend::Postgres)
}
