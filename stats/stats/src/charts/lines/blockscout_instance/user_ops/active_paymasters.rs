//! Active paymasters on each day.

use std::{collections::HashSet, ops::Range};

use crate::chart_prelude::*;

use blockscout_db::entity::user_operations;

use super::count_distinct_in_user_ops;

pub struct ActivePaymastersStatement;
impl_db_choice!(ActivePaymastersStatement, UsePrimaryDB);

impl StatementFromRange for ActivePaymastersStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _completed_migrations: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        count_distinct_in_user_ops(user_operations::Column::Paymaster.into_column_ref(), range)
    }
}

pub type ActivePaymastersRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        ActivePaymastersStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "activePaymasters".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::LEAST_RESTRICTIVE
            .with_blockscout(BlockscoutIndexingStatus::BlocksIndexed)
            .with_user_ops(UserOpsIndexingStatus::PastOperationsIndexed)
    }
}

pub type ActivePaymasters =
    DirectVecLocalDbChartSource<ActivePaymastersRemote, Batch30Days, Properties>;

#[cfg(test)]
mod tests {
    use crate::tests::simple_test::simple_test_chart;

    use super::ActivePaymasters;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_paymasters() {
        simple_test_chart::<ActivePaymasters>(
            "update_active_paymasters",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "1"),
                ("2022-11-11", "1"),
                ("2022-11-12", "1"),
                ("2022-12-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }
}
