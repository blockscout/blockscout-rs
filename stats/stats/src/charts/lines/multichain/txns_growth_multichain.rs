use std::{collections::HashSet, ops::Range};

use crate::{
    charts::db_interaction::read::QueryFullIndexerTimestampRange, data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
                resolutions::last_value::LastValueLowerResolution,
            },
            local_db::{
                parameters::update::batching::parameters::{
                    Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
                }, DirectVecLocalDbChartSource
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::IndexerMigrations,
    }, define_and_impl_resolution_properties, types::timespans::{Month, Week, Year}, utils::sql_with_range_filter_opt, ChartKey, ChartProperties, Named
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct TxnsGrowthMultichainStatement;

impl StatementFromRange for TxnsGrowthMultichainStatement {
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
                    SUM(c.total_transactions_number)::TEXT AS value
                FROM counters_global_imported as c
                WHERE
                    c.total_transactions_number IS NOT NULL
                    {filter}
                GROUP BY date
            "#,
            [],
            "c.date::timestamp",
            range
        )
    }
}

pub type TxnsGrowthMultichainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        TxnsGrowthMultichainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "txnsGrowthMultichain".into()
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

pub type TxnsGrowthMultichain =
    DirectVecLocalDbChartSource<TxnsGrowthMultichainRemote, Batch30Days, Properties>;
pub type TxnsGrowthMultichainInt = MapParseTo<StripExt<TxnsGrowthMultichain>, i64>;
pub type TxnsGrowthMultichainWeekly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<TxnsGrowthMultichainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type TxnsGrowthMultichainMonthly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<TxnsGrowthMultichainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type TxnsGrowthMultichainMonthlyInt = MapParseTo<StripExt<TxnsGrowthMultichainMonthly>, i64>;
pub type TxnsGrowthMultichainYearly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<TxnsGrowthMultichainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_multichain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth_multichain() {
        simple_test_chart_multichain::<TxnsGrowthMultichain>(
            "update_txns_growth_multichain",
            vec![
                ("2022-06-28", "66"),
                ("2022-07-01", "76"),
                ("2022-08-04", "101"),
                ("2022-08-05", "150"),
                ("2022-08-06", "210"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth_multichain_weekly() {
        simple_test_chart_multichain::<TxnsGrowthMultichainWeekly>(
            "update_txns_growth_multichain_weekly",
            vec![("2022-06-27", "76"), ("2022-08-01", "210")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth_multichain_monthly() {
        simple_test_chart_multichain::<TxnsGrowthMultichainMonthly>(
            "update_txns_growth_multichain_monthly",
            vec![
                ("2022-06-01", "66"),
                ("2022-07-01", "76"),
                ("2022-08-01", "210"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth_multichain_yearly() {
        simple_test_chart_multichain::<TxnsGrowthMultichainYearly>(
            "update_txns_growth_multichain_yearly",
            vec![("2022-01-01", "210")],
        )
        .await;
    }
}
