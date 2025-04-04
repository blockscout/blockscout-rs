//! Active bundlers on each day.

use std::{collections::HashSet, ops::Range};

use crate::{
    charts::db_interaction::read::QueryAllBlockTimestampRange,
    data_source::{
        kinds::{
            local_db::{
                parameters::update::batching::parameters::Batch30Days, DirectVecLocalDbChartSource,
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::BlockscoutMigrations,
    },
    indexing_status::{BlockscoutIndexingStatus, IndexingStatus, UserOpsIndexingStatus},
    ChartKey, ChartProperties, Named,
};

use blockscout_db::entity::user_operations;
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use migration::IntoColumnRef;
use sea_orm::Statement;

use super::count_distinct_in_user_ops;

pub struct ActiveBundlersStatement;

impl StatementFromRange for ActiveBundlersStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _completed_migrations: &BlockscoutMigrations,
        _: &HashSet<ChartKey>,
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
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus {
            blockscout: BlockscoutIndexingStatus::BlocksIndexed,
            user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
        }
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
