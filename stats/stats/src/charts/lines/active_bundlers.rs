//! Active bundlers on each day.

use std::ops::Range;

use crate::{
    charts::db_interaction::{read::QueryAllBlockTimestampRange, utils::datetime_range_filter},
    data_source::{
        kinds::{
            local_db::{
                parameters::update::batching::parameters::Batch30Days, DirectVecLocalDbChartSource,
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::BlockscoutMigrations,
    },
    ChartProperties, Named,
};

use blockscout_db::entity::{blocks, user_operations};
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use migration::{Alias, Expr, Func, IntoColumnRef, IntoIden, SimpleExpr};
use sea_orm::{
    ColumnTrait, EntityTrait, IntoIdentity, IntoSimpleExpr, Order, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Statement,
};

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

pub struct ActiveBundlersStatement;

impl StatementFromRange for ActiveBundlersStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        count_distinct_in_user_ops(user_operations::Column::Bundler.into_column_ref(), range)
    }
}

pub type ActiveBundlersRemote = RemoteDatabaseSource<
    PullAllWithAndSort<ActiveBundlersStatement, NaiveDate, String, QueryAllBlockTimestampRange>,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "activeBundlers".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type ActiveBundlers =
    DirectVecLocalDbChartSource<ActiveBundlersRemote, Batch30Days, Properties>;

#[cfg(test)]
mod tests {
    use crate::tests::simple_test::simple_test_chart;

    use super::ActiveBundlers;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_bundlers() {
        simple_test_chart::<ActiveBundlers>(
            "update_active_bundlers",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "2"),
                ("2022-11-11", "2"),
                ("2022-11-12", "1"),
                ("2022-12-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }
}
