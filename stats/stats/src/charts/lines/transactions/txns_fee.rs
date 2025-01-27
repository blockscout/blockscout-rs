//! Total transaction fees for an interval

use std::ops::Range;

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
    types::timespans::{Month, Week, Year},
    utils::{produce_filter_and_values, sql_with_range_filter_opt},
    ChartProperties, Named,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

const ETHER: i64 = i64::pow(10, 18);

pub struct TxnsFeeStatement;

impl StatementFromRange for TxnsFeeStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        if completed_migrations.denormalization {
            // TODO: consider supporting such case in macro ?
            let mut args = vec![ETHER.into()];
            let (tx_filter, new_args) =
                produce_filter_and_values(range.clone(), "t.block_timestamp", args.len() + 1);
            args.extend(new_args);
            let (block_filter, new_args) =
                produce_filter_and_values(range.clone(), "b.timestamp", args.len() + 1);
            args.extend(new_args);
            let sql = format!(
                r#"
                    SELECT
                        DATE(b.timestamp) as date,
                        (SUM(
                            t_filtered.gas_used *
                            COALESCE(
                                t_filtered.gas_price,
                                b.base_fee_per_gas + LEAST(
                                    t_filtered.max_priority_fee_per_gas,
                                    t_filtered.max_fee_per_gas - b.base_fee_per_gas
                                )
                            )
                        ) / $1)::FLOAT as value
                    FROM (
                        SELECT * from transactions t
                        WHERE
                            t.block_consensus = true AND
                            t.block_timestamp != to_timestamp(0) {tx_filter}
                    ) as t_filtered
                    JOIN blocks       b ON t_filtered.block_hash = b.hash
                    WHERE
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true {block_filter}
                    GROUP BY DATE(b.timestamp)
                "#,
            );
            Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
        } else {
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
}

pub type TxnsFeeRemote = RemoteDatabaseSource<
    PullAllWithAndSort<TxnsFeeStatement, NaiveDate, f64, QueryAllBlockTimestampRange>,
>;

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
pub type TxnsFeeFloat = MapParseTo<StripExt<TxnsFee>, f64>;
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
pub type TxnsFeeMonthlyFloat = MapParseTo<StripExt<TxnsFeeMonthly>, f64>;
pub type TxnsFeeYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<TxnsFeeMonthlyFloat, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_with_migration_variants;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee() {
        simple_test_chart_with_migration_variants::<TxnsFee>(
            "update_txns_fee",
            vec![
                ("2022-11-09", "0.000047185185138"),
                ("2022-11-10", "0.000613407406794"),
                ("2022-11-11", "0.001226814813588"),
                ("2022-11-12", "0.000802148147346"),
                ("2022-12-01", "0.000896518517622"),
                ("2023-01-01", "0.000023592592569"),
                ("2023-02-01", "0.001061666665605"),
                ("2023-03-01", "0.000023592592569"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_weekly() {
        simple_test_chart_with_migration_variants::<TxnsFeeWeekly>(
            "update_txns_fee_weekly",
            vec![
                ("2022-11-07", "0.002689555552866"),
                ("2022-11-28", "0.000896518517622"),
                ("2022-12-26", "0.000023592592569"),
                ("2023-01-30", "0.001061666665605"),
                ("2023-02-27", "0.000023592592569"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_monthly() {
        simple_test_chart_with_migration_variants::<TxnsFeeMonthly>(
            "update_txns_fee_monthly",
            vec![
                ("2022-11-01", "0.002689555552866"),
                ("2022-12-01", "0.000896518517622"),
                ("2023-01-01", "0.000023592592569"),
                ("2023-02-01", "0.001061666665605"),
                ("2023-03-01", "0.000023592592569"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_yearly() {
        simple_test_chart_with_migration_variants::<TxnsFeeYearly>(
            "update_txns_fee_yearly",
            vec![
                ("2022-01-01", "0.003586074070488"),
                ("2023-01-01", "0.001108851850743"),
            ],
        )
        .await;
    }
}
