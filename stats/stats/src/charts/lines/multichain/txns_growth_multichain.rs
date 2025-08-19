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
    }, define_and_impl_resolution_properties, types::timespans::{Month, Week, Year}, utils::{produce_filter_and_values, sql_with_range_filter_opt}, ChartKey, ChartProperties, Named
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DatabaseBackend, DbBackend, Statement};

pub struct TxnsGrowthMultichainStatement;

impl StatementFromRange for TxnsGrowthMultichainStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        
        let to_timestamp = range.map(|r| r.end).unwrap_or_else(Utc::now);
        let sql = format!(
            r#"
                SELECT
                    $1::date AS date,
                    COALESCE(SUM(sub.total_transactions_number), 0)::TEXT AS value
                FROM (
                    SELECT DISTINCT ON (c.chain_id)
                        c.chain_id,
                        c.total_transactions_number
                    FROM counters_global_imported c
                    WHERE
                        c.date < $1
                        AND c.total_transactions_number IS NOT NULL
                    ORDER BY c.chain_id, c.date DESC
                ) sub;
            "#,
        );
        Statement::from_sql_and_values(DbBackend::Postgres, sql, vec![to_timestamp.into()])
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
                ("2022-12-28", "66"),
                ("2023-01-01", "76"),
                ("2023-02-02", "101"),
                ("2023-02-03", "150"),
                ("2023-02-04", "210"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth_multichain_weekly() {
        simple_test_chart_multichain::<TxnsGrowthMultichainWeekly>(
            "update_txns_growth_multichain_weekly",
            vec![("2022-12-26", "76"), ("2023-01-30", "210")],
        )
        .await;
    }

    // ("2023-02-04", 1, 10, 46, 170),
    // ("2023-02-04", 2, 20, 55, 300),
    // ("2023-02-04", 3, 30, 109, 450),

    // ("2023-02-03", 1, 4, 36, 160),
    // ("2023-02-03", 2, 7, 35, 290),
    // ("2023-02-03", 3, 38, 79, 422),

    // ("2023-02-02", 1, 18, 32, 155),
    // ("2023-02-02", 2, 3, 28, 250),
    // ("2023-02-02", 3, 4, 41, 420),

    // ("2023-01-01", 1, 3, 14, 150),
    // ("2023-01-01", 2, 3, 25, 250),
    // ("2023-01-01", 3, 4, 37, 350),

    // ("2022-12-28", 1, 11, 11, 111),
    // ("2022-12-28", 2, 22, 22, 222),
    // ("2022-12-28", 3, 33, 33, 333),

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth_multichain_monthly() {
        simple_test_chart_multichain::<TxnsGrowthMultichainMonthly>(
            "update_txns_growth_multichain_monthly",
            vec![
                ("2022-12-01", "66"),
                ("2023-01-01", "76"),
                ("2023-02-01", "210"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth_multichain_yearly() {
        simple_test_chart_multichain::<TxnsGrowthMultichainYearly>(
            "update_txns_growth_multichain_yearly",
            vec![("2022-01-01", "66"), ("2023-01-01", "210")],
        )
        .await;
    }
}
