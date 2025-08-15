use std::{collections::HashSet, ops::Range};

use crate::{
    ChartKey, ChartProperties, Named,
    charts::db_interaction::read::QueryFullIndexerTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
                resolutions::sum::SumLowerResolution,
            },
            local_db::{
                DirectVecLocalDbChartSource,
                parameters::update::batching::parameters::{
                    Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
                },
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::IndexerMigrations,
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct NewTxnsMultichainStatement;

impl StatementFromRange for NewTxnsMultichainStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    c.date,
                    SUM(c.daily_transactions_number)::TEXT AS value
                FROM counters_global_imported as c
                WHERE
                    c.daily_transactions_number IS NOT NULL
                    {filter}
                GROUP BY date
            "#,
            [],
            "c.date::timestamp",
            range
        )
    }
}

pub type NewTxnsMultichainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        NewTxnsMultichainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTxnsMultichain".into()
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
        MonthlyProperties: Month,
        YearlyProperties: Year,
    },
    base_impl: Properties
);

pub type NewTxnsMultichain =
    DirectVecLocalDbChartSource<NewTxnsMultichainRemote, Batch30Days, Properties>;
pub type NewTxnsMultichainInt = MapParseTo<StripExt<NewTxnsMultichain>, i64>;
pub type NewTxnsMultichainWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsMultichainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewTxnsMultichainMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsMultichainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewTxnsMultichainMonthlyInt = MapParseTo<StripExt<NewTxnsMultichainMonthly>, i64>;
pub type NewTxnsMultichainYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsMultichainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_multichain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_multichain() {
        simple_test_chart_multichain::<NewTxnsMultichain>(
            "update_new_txns_multichain",
            vec![
                ("2022-06-28", "66"),
                ("2022-07-01", "10"),
                ("2022-08-04", "25"),
                ("2022-08-05", "49"),
                ("2022-08-06", "60"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txs_multichain_weekly() {
        simple_test_chart_multichain::<NewTxnsMultichainWeekly>(
            "update_new_txs_multichain_weekly",
            vec![("2022-06-27", "76"), ("2022-08-01", "134")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txs_multichain_monthly() {
        simple_test_chart_multichain::<NewTxnsMultichainMonthly>(
            "update_new_txs_multichain_monthly",
            vec![
                ("2022-06-01", "66"),
                ("2022-07-01", "10"),
                ("2022-08-01", "134"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txs_multichain_yearly() {
        simple_test_chart_multichain::<NewTxnsMultichainYearly>(
            "update_new_txs_multichain_yearly",
            vec![("2022-01-01", "210")],
        )
        .await;
    }
}
