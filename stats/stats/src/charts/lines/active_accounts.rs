//! Active accounts on each day.

use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            local_db::{
                parameters::update::batching::parameters::Batch30Days, DirectVecLocalDbChartSource,
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::BlockscoutMigrations,
    },
    utils::sql_with_range_filter_opt,
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct ActiveAccountsStatement;

impl StatementFromRange for ActiveAccountsStatement {
    fn get_statement(
        range: Option<Range<DateTimeUtc>>,
        completed_migrations: &BlockscoutMigrations,
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

pub type ActiveAccountsRemote =
    RemoteDatabaseSource<PullAllWithAndSort<ActiveAccountsStatement, NaiveDate, String>>;

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

pub type ActiveAccounts =
    DirectVecLocalDbChartSource<ActiveAccountsRemote, Batch30Days, Properties>;

#[cfg(test)]
mod tests {
    use crate::tests::simple_test::simple_test_chart_with_migration_variants;

    use super::ActiveAccounts;

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
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }
}
