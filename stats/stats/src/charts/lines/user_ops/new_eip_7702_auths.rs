use std::{collections::HashSet, ops::Range};

use crate::{
    charts::db_interaction::read::QueryAllBlockTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
                resolutions::sum::SumLowerResolution,
            },
            local_db::{
                parameters::update::batching::parameters::{
                    Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
                },
                DirectVecLocalDbChartSource,
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::BlockscoutMigrations,
    },
    define_and_impl_resolution_properties,
    indexing_status::{BlockscoutIndexingStatus, IndexingStatus, UserOpsIndexingStatus},
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartKey, ChartProperties, Named,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct NewEip7702AuthsStatement;

impl StatementFromRange for NewEip7702AuthsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        if completed_migrations.denormalization {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                SELECT
                    date(transactions.block_timestamp) as date,
                    COUNT(*)::TEXT as value
                FROM signed_authorizations
                INNER JOIN transactions ON signed_authorizations.transaction_hash = transactions.hash
                WHERE
                    transactions.block_consensus = true {filter}
                GROUP BY date
                ORDER BY date ASC;
            "#,
                [],
                "transactions.block_timestamp",
                range
            )
        } else {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        date(blocks.timestamp) as date,
                        COUNT(*)::TEXT as value
                    FROM signed_authorizations
                    INNER JOIN transactions ON signed_authorizations.transaction_hash = transactions.hash
                    INNER JOIN blocks ON transactions.block_hash = blocks.hash
                    WHERE
                        blocks.consensus = true {filter}
                    GROUP BY date
                    ORDER BY date ASC;
                "#,
                [],
                "blocks.timestamp",
                range
            )
        }
    }
}

pub type NewEip7702AuthsRemote = RemoteDatabaseSource<
    PullAllWithAndSort<NewEip7702AuthsStatement, NaiveDate, String, QueryAllBlockTimestampRange>,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newEip7702Auths".into()
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
            user_ops: UserOpsIndexingStatus::IndexingPastOperations,
        }
    }
}

define_and_impl_resolution_properties!(
    define_and_impl: {
        WeeklyProperties: Week,
        MonthlyProperties: Month,
        YearlyProperties: Year,
    },
    base_impl: Properties
);

pub type NewEip7702Auths =
    DirectVecLocalDbChartSource<NewEip7702AuthsRemote, Batch30Days, Properties>;
pub type NewEip7702AuthsInt = MapParseTo<StripExt<NewEip7702Auths>, i64>;
pub type NewEip7702AuthsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewEip7702AuthsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewEip7702AuthsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewEip7702AuthsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewEip7702AuthsMonthlyInt = MapParseTo<StripExt<NewEip7702AuthsMonthly>, i64>;
pub type NewEip7702AuthsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewEip7702AuthsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use crate::tests::simple_test::simple_test_chart_with_migration_variants;

    use super::*;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_eip_7702_auths() {
        simple_test_chart_with_migration_variants::<NewEip7702Auths>(
            "update_new_eip_7702_auths",
            vec![
                ("2022-11-10", "1"),
                ("2022-11-11", "2"),
                ("2023-03-01", "3"),
            ],
        )
        .await;
    }
}
