//! Average fee per transaction

use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString},
                resolutions::average::AverageLowerResolution,
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

use super::new_txns::{NewTxnsInt, NewTxnsMonthlyInt};

const ETHER: i64 = i64::pow(10, 18);

pub struct AverageTxnFeeStatement;

impl StatementFromRange for AverageTxnFeeStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>, _: &BlockscoutMigrations) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    DATE(b.timestamp) as date,
                    (AVG(
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

pub type AverageTxnFeeRemote =
    RemoteDatabaseSource<PullAllWithAndSort<AverageTxnFeeStatement, NaiveDate, f64>>;

pub type AverageTxnFeeRemoteString = MapToString<AverageTxnFeeRemote>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "averageTxnFee".into()
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

pub type AverageTxnFee =
    DirectVecLocalDbChartSource<AverageTxnFeeRemoteString, Batch30Days, Properties>;
pub type AverageTxnFeeWeekly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageTxnFee, f64>, NewTxnsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AverageTxnFeeMonthly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageTxnFee, f64>, NewTxnsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type AverageTxnFeeYearly = DirectVecLocalDbChartSource<
    MapToString<
        AverageLowerResolution<MapParseTo<AverageTxnFeeMonthly, f64>, NewTxnsMonthlyInt, Year>,
    >,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txn_fee() {
        simple_test_chart::<AverageTxnFee>(
            "update_average_txn_fee",
            vec![
                ("2022-11-09", "0.0000094370370276"),
                ("2022-11-10", "0.00004128703699575"),
                ("2022-11-11", "0.0000690925925235"),
                ("2022-11-12", "0.0001226814813588"),
                ("2022-12-01", "0.0001368370369002"),
                ("2023-01-01", "0.000023592592569"),
                ("2023-02-01", "0.0002005370368365"),
                ("2023-03-01", "0.000023592592569"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txn_fee_weekly() {
        simple_test_chart::<AverageTxnFeeWeekly>(
            "update_average_txn_fee_weekly",
            vec![
                ("2022-11-07", "0.00005898148142249999"),
                ("2022-11-28", "0.0001368370369002"),
                ("2022-12-26", "0.000023592592569"),
                ("2023-01-30", "0.0002005370368365"),
                ("2023-02-27", "0.000023592592569"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txn_fee_monthly() {
        simple_test_chart::<AverageTxnFeeMonthly>(
            "update_average_txn_fee_monthly",
            vec![
                ("2022-11-01", "0.00005898148142249999"),
                ("2022-12-01", "0.0001368370369002"),
                ("2023-01-01", "0.000023592592569"),
                ("2023-02-01", "0.0002005370368365"),
                ("2023-03-01", "0.000023592592569"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txn_fee_yearly() {
        simple_test_chart::<AverageTxnFeeYearly>(
            "update_average_txn_fee_yearly",
            vec![
                ("2022-01-01", "0.00006847606135880486"),
                ("2023-01-01", "0.000141555555414"),
            ],
        )
        .await;
    }
}
