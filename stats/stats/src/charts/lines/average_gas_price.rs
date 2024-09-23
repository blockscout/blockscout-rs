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
    utils::{produce_filter_and_values, sql_with_range_filter_opt},
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

use super::new_txns::{NewTxnsInt, NewTxnsMonthlyInt};

const GWEI: i64 = 1_000_000_000;

pub struct AverageGasPriceStatement;

impl StatementFromRange for AverageGasPriceStatement {
    fn get_statement(
        range: Option<Range<DateTimeUtc>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        if completed_migrations.denormalization {
            // TODO: consider supporting such case in macro ?
            let mut args = vec![GWEI.into()];
            let (tx_filter, new_args) =
                produce_filter_and_values(range.clone(), "t.block_timestamp", args.len() + 1);
            args.extend(new_args);
            let (block_filter, new_args) =
                produce_filter_and_values(range.clone(), "b.timestamp", args.len() + 1);
            args.extend(new_args);
            let sql = format!(
                r#"

                    SELECT
                        b.timestamp::date as date,
                        (AVG(
                            COALESCE(
                                t_filtered.gas_price,
                                b.base_fee_per_gas + LEAST(
                                    t_filtered.max_priority_fee_per_gas,
                                    t_filtered.max_fee_per_gas - b.base_fee_per_gas
                                )
                            )
                        ) / $1)::float as value
                    FROM (
                        SELECT * from transactions t
                        WHERE
                            t.block_consensus = true AND
                            t.block_timestamp != to_timestamp(0) {tx_filter}
                    ) as t_filtered
                    JOIN blocks b ON t_filtered.block_hash = b.hash
                    WHERE
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true {block_filter}
                    GROUP BY date
                "#,
            );
            Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
        } else {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        blocks.timestamp::date as date,
                        (AVG(
                            COALESCE(
                                transactions.gas_price,
                                blocks.base_fee_per_gas + LEAST(
                                    transactions.max_priority_fee_per_gas,
                                    transactions.max_fee_per_gas - blocks.base_fee_per_gas
                                )
                            )
                        ) / $1)::float as value
                    FROM transactions
                    JOIN blocks ON transactions.block_hash = blocks.hash
                    WHERE
                        blocks.timestamp != to_timestamp(0) AND
                        blocks.consensus = true {filter}
                    GROUP BY date
                "#,
                [GWEI.into()],
                "blocks.timestamp",
                range,
            )
        }
    }
}

pub type AverageGasPriceRemote =
    RemoteDatabaseSource<PullAllWithAndSort<AverageGasPriceStatement, NaiveDate, f64>>;

pub type AverageGasPriceRemoteString = MapToString<AverageGasPriceRemote>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "averageGasPrice".into()
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

pub type AverageGasPrice =
    DirectVecLocalDbChartSource<AverageGasPriceRemoteString, Batch30Days, Properties>;
pub type AverageGasPriceWeekly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageGasPrice, f64>, NewTxnsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AverageGasPriceMonthly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageGasPrice, f64>, NewTxnsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type AverageGasPriceYearly = DirectVecLocalDbChartSource<
    MapToString<
        AverageLowerResolution<MapParseTo<AverageGasPriceMonthly, f64>, NewTxnsMonthlyInt, Year>,
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
    async fn update_average_gas_price() {
        simple_test_chart_with_migration_variants::<AverageGasPrice>(
            "update_average_gas_price",
            vec![
                ("2022-11-09", "0.4493827156"),
                ("2022-11-10", "1.96604938075"),
                ("2022-11-11", "3.2901234535"),
                ("2022-11-12", "5.8419753028"),
                ("2022-12-01", "6.5160493762"),
                ("2023-01-01", "1.123456789"),
                ("2023-02-01", "9.5493827065"),
                ("2023-03-01", "1.123456789"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_price_weekly() {
        simple_test_chart_with_migration_variants::<AverageGasPriceWeekly>(
            "update_average_gas_price_weekly",
            vec![
                ("2022-11-07", "2.8086419725000003"),
                ("2022-11-28", "6.5160493762"),
                ("2022-12-26", "1.123456789"),
                ("2023-01-30", "9.5493827065"),
                ("2023-02-27", "1.123456789"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_price_monthly() {
        simple_test_chart_with_migration_variants::<AverageGasPriceMonthly>(
            "update_average_gas_price_monthly",
            vec![
                ("2022-11-01", "2.8086419725000003"),
                ("2022-12-01", "6.5160493762"),
                ("2023-01-01", "1.123456789"),
                ("2023-02-01", "9.5493827065"),
                ("2023-03-01", "1.123456789"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_price_yearly() {
        simple_test_chart_with_migration_variants::<AverageGasPriceYearly>(
            "update_average_gas_price_yearly",
            vec![
                ("2022-01-01", "3.260764826609756"),
                ("2023-01-01", "6.740740734"),
            ],
        )
        .await;
    }
}
