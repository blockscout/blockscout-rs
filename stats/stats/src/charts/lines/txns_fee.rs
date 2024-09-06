//! Total transaction fees for an interval

use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString},
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
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

const ETHER: i64 = i64::pow(10, 18);

pub struct TxnsFeeStatement;

impl StatementFromRange for TxnsFeeStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>, _: &BlockscoutMigrations) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    DATE(b.timestamp) as date,
                    (SUM(
                        t.gas_used *
                        COALESCE(
                            t.gas_price,
                            b.base_fee_per_gas + LEAST(
                                t.max_priority_fee_per_gas,
                                t.max_fee_per_gas - b.base_fee_per_gas
                            )
                        )
                    ) / $1)::FLOAT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE
                    b.timestamp != to_timestamp(0) AND
                    b.consensus = true {filter}
                GROUP BY DATE(b.timestamp)
            "#,
            [ETHER.into()],
            "b.timestamp",
            range
        )
    }
}

pub type TxnsFeeRemote = RemoteDatabaseSource<PullAllWithAndSort<TxnsFeeStatement, NaiveDate, f64>>;

pub type TxnsFeeRemoteString = MapToString<TxnsFeeRemote>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "txnsFee".into()
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

pub type TxnsFee = DirectVecLocalDbChartSource<TxnsFeeRemoteString, Batch30Days, Properties>;
pub type TxnsFeeFloat = MapParseTo<TxnsFee, f64>;
pub type TxnsFeeWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<TxnsFeeFloat, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type TxnsFeeMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<TxnsFeeFloat, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type TxnsFeeMonthlyFloat = MapParseTo<TxnsFeeMonthly, f64>;
pub type TxnsFeeYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<TxnsFeeMonthlyFloat, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee() {
        simple_test_chart::<TxnsFee>(
            "update_txns_fee",
            vec![
                ("2022-11-09", "0.000047185185138"),
                ("2022-11-10", "0.000495444443949"),
                ("2022-11-11", "0.000967296295329"),
                ("2022-11-12", "0.000613407406794"),
                ("2022-12-01", "0.000684185184501"),
                ("2023-01-01", "0.000023592592569"),
                ("2023-02-01", "0.000802148147346"),
                ("2023-03-01", "0.000023592592569"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_weekly() {
        simple_test_chart::<TxnsFeeWeekly>(
            "update_txns_fee_weekly",
            vec![
                ("2022-11-07", "0.00212333333121"),
                ("2022-11-28", "0.000684185184501"),
                ("2022-12-26", "0.000023592592569"),
                ("2023-01-30", "0.000802148147346"),
                ("2023-02-27", "0.000023592592569"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_monthly() {
        simple_test_chart::<TxnsFeeMonthly>(
            "update_txns_fee_monthly",
            vec![
                ("2022-11-01", "0.00212333333121"),
                ("2022-12-01", "0.000684185184501"),
                ("2023-01-01", "0.000023592592569"),
                ("2023-02-01", "0.000802148147346"),
                ("2023-03-01", "0.000023592592569"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_yearly() {
        simple_test_chart::<TxnsFeeYearly>(
            "update_txns_fee_yearly",
            vec![
                ("2022-01-01", "0.002807518515711"),
                ("2023-01-01", "0.0008493333324839999"),
            ],
        )
        .await;
    }
}
