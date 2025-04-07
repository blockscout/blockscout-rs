//! Average fee per transaction

use std::{collections::HashSet, ops::Range};

use crate::{
    charts::db_interaction::read::QueryAllBlockTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
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
    utils::{produce_filter_and_values, sql_with_range_filter_opt},
    ChartKey, ChartProperties, Named,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

use super::new_txns::{NewTxnsInt, NewTxnsMonthlyInt};

const ETHER: i64 = i64::pow(10, 18);

pub struct AverageTxnFeeStatement;

impl StatementFromRange for AverageTxnFeeStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
        _: &HashSet<ChartKey>,
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
                        (AVG(
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
}

pub type AverageTxnFeeRemote = RemoteDatabaseSource<
    PullAllWithAndSort<AverageTxnFeeStatement, NaiveDate, f64, QueryAllBlockTimestampRange>,
>;

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
type AverageTxnFeeS = StripExt<AverageTxnFee>;
pub type AverageTxnFeeWeekly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageTxnFeeS, f64>, NewTxnsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AverageTxnFeeMonthly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageTxnFeeS, f64>, NewTxnsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
type AverageTxnFeeMonthlyS = StripExt<AverageTxnFeeMonthly>;
pub type AverageTxnFeeYearly = DirectVecLocalDbChartSource<
    MapToString<
        AverageLowerResolution<MapParseTo<AverageTxnFeeMonthlyS, f64>, NewTxnsMonthlyInt, Year>,
    >,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_with_migration_variants;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txn_fee() {
        simple_test_chart_with_migration_variants::<AverageTxnFee>(
            "update_average_txn_fee",
            vec![
                ("2022-11-09", "0.000007864197523"),
                ("2022-11-10", "0.000043814814771"),
                ("2022-11-11", "0.00007667592584925"),
                ("2022-11-12", "0.000133691357891"),
                ("2022-12-01", "0.000149419752937"),
                ("2023-01-01", "0.000023592592569"),
                ("2023-02-01", "0.000212333333121"),
                ("2023-03-01", "0.0000117962962845"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txn_fee_weekly() {
        simple_test_chart_with_migration_variants::<AverageTxnFeeWeekly>(
            "update_average_txn_fee_weekly",
            vec![
                ("2022-11-07", "0.000064037036973"),
                ("2022-11-28", "0.000149419752937"),
                ("2022-12-26", "0.000023592592569"),
                ("2023-01-30", "0.000212333333121"),
                ("2023-02-27", "0.0000117962962845"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txn_fee_monthly() {
        simple_test_chart_with_migration_variants::<AverageTxnFeeMonthly>(
            "update_average_txn_fee_monthly",
            vec![
                ("2022-11-01", "0.000064037036973"),
                ("2022-12-01", "0.000149419752937"),
                ("2023-01-01", "0.000023592592569"),
                ("2023-02-01", "0.000212333333121"),
                ("2023-03-01", "0.0000117962962845"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txn_fee_yearly() {
        simple_test_chart_with_migration_variants::<AverageTxnFeeYearly>(
            "update_average_txn_fee_yearly",
            vec![
                ("2022-01-01", "0.0000747098764685"),
                ("2023-01-01", "0.000138606481342875"),
            ],
        )
        .await;
    }
}
