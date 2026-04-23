//! Active accounts on each day.

use std::{collections::HashSet, ops::Range};

use crate::chart_prelude::*;

pub struct ActiveAccountsStatement;
impl_db_choice!(ActiveAccountsStatement, UsePrimaryDB);

impl StatementFromRange for ActiveAccountsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &IndexerMigrations,
        _enabled_update_charts_recursive: &HashSet<ChartKey>,
    ) -> Statement {
        if completed_migrations.denormalization {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        DATE(block_timestamp) as date,
                        COUNT(DISTINCT from_address_hash)::TEXT as value
                    FROM transactions
                    WHERE
                        block_timestamp != to_timestamp(0) AND
                        block_consensus = true {filter}
                    GROUP BY date(block_timestamp);
                "#,
                [],
                "block_timestamp",
                range
            )
        } else {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        DATE(blocks.timestamp) as date,
                        COUNT(DISTINCT from_address_hash)::TEXT as value
                    FROM transactions
                    JOIN blocks on transactions.block_hash = blocks.hash
                    WHERE
                        blocks.timestamp != to_timestamp(0) AND
                        blocks.consensus = true {filter}
                    GROUP BY date(blocks.timestamp);
                "#,
                [],
                "blocks.timestamp",
                range
            )
        }
    }
}

pub type ActiveAccountsRemote = RemoteDatabaseSource<
    PullAllWithAndSort<ActiveAccountsStatement, NaiveDate, String, QueryFullIndexerTimestampRange>,
>;

pub struct ActiveAccountsForTimespan;
impl_db_choice!(ActiveAccountsForTimespan, UsePrimaryDB);

impl StatementFromTimespan for ActiveAccountsForTimespan {
    fn get_statement(
        point: Range<DateTime<Utc>>,
        completed_migrations: &IndexerMigrations,
    ) -> Statement {
        if completed_migrations.denormalization {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        COUNT(DISTINCT from_address_hash)::TEXT as value
                    FROM transactions
                    WHERE
                        block_timestamp != to_timestamp(0) AND
                        block_consensus = true {filter};
                "#,
                [],
                "block_timestamp",
                Some(point)
            )
        } else {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        COUNT(DISTINCT from_address_hash)::TEXT as value
                    FROM transactions
                    JOIN blocks on transactions.block_hash = blocks.hash
                    WHERE
                        blocks.timestamp != to_timestamp(0) AND
                        blocks.consensus = true {filter};
                "#,
                [],
                "blocks.timestamp",
                Some(point)
            )
        }
    }
}

pub type ActiveAccountsRemoteWeekly = RemoteDatabaseSource<
    PullEachWith<ActiveAccountsForTimespan, Week, String, QueryFullIndexerTimestampRange>,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "activeAccounts".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

define_and_impl_resolution_properties!(
    define_and_impl: {
        WeeklyProperties: Week,
        // MonthlyProperties: Month,
        // YearlyProperties: Year,
    },
    base_impl: Properties
);

pub type ActiveAccounts =
    DirectVecLocalDbChartSource<ActiveAccountsRemote, Batch30Days, Properties>;

pub type ActiveAccountsWeekly = DirectVecLocalDbChartSource<
    MapToString<FilterDeducible<MapParseTo<ActiveAccountsRemoteWeekly, i64>, WeeklyProperties>>,
    Batch30Weeks,
    WeeklyProperties,
>;

#[cfg(test)]
mod tests {
    use crate::tests::simple_test::simple_test_chart_with_migration_variants;

    use super::{ActiveAccounts, ActiveAccountsWeekly};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_accounts() {
        simple_test_chart_with_migration_variants::<ActiveAccounts>(
            "update_active_accounts",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "3"),
                ("2022-11-11", "4"),
                ("2022-11-12", "1"),
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
                ("2023-03-01", "2"),
            ],
        )
        .await;
    }
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_accounts_weekly() {
        simple_test_chart_with_migration_variants::<ActiveAccountsWeekly>(
            "update_active_accounts_weekly",
            vec![
                ("2022-11-07", "8"),
                ("2022-11-28", "1"),
                ("2022-12-26", "1"),
                ("2023-01-30", "1"),
                ("2023-02-27", "2"),
            ],
        )
        .await;
    }
}
