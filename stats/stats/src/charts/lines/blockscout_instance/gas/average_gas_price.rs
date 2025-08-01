use std::{collections::HashSet, ops::Range};

use crate::{
    ChartKey, ChartProperties, Named,
    charts::db_interaction::read::QueryAllBlockTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
                resolutions::average::AverageLowerResolution,
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
    lines::{NewTxnsInt, NewTxnsMonthlyInt},
    types::timespans::{Month, Week, Year},
    utils::{produce_filter_and_values, sql_with_range_filter_opt},
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

const GWEI: i64 = 1_000_000_000;

pub struct AverageGasPriceStatement;

impl StatementFromRange for AverageGasPriceStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &IndexerMigrations,
        _: &HashSet<ChartKey>,
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

pub type AverageGasPriceRemote = RemoteDatabaseSource<
    PullAllWithAndSort<AverageGasPriceStatement, NaiveDate, f64, QueryAllBlockTimestampRange>,
>;

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
type AverageGasPriceS = StripExt<AverageGasPrice>;
pub type AverageGasPriceWeekly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageGasPriceS, f64>, NewTxnsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AverageGasPriceMonthly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageGasPriceS, f64>, NewTxnsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
type AverageGasPriceMonthlyS = StripExt<AverageGasPriceMonthly>;
pub type AverageGasPriceYearly = DirectVecLocalDbChartSource<
    MapToString<
        AverageLowerResolution<MapParseTo<AverageGasPriceMonthlyS, f64>, NewTxnsMonthlyInt, Year>,
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
                ("2022-11-09", "0.37448559633333334"),
                ("2022-11-10", "2.086419751"),
                ("2022-11-11", "3.65123456425"),
                ("2022-11-12", "6.366255137666666"),
                ("2022-12-01", "7.115226330333333"),
                ("2023-01-01", "1.123456789"),
                ("2023-02-01", "10.111111101"),
                ("2023-03-01", "0.5617283945"),
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
                ("2022-11-07", "3.049382713"),
                ("2022-11-28", "7.115226330333333"),
                ("2022-12-26", "1.123456789"),
                ("2023-01-30", "10.111111101"),
                ("2023-02-27", "0.5617283945"),
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
                ("2022-11-01", "3.049382713"),
                ("2022-12-01", "7.115226330333333"),
                ("2023-01-01", "1.123456789"),
                ("2023-02-01", "10.111111101"),
                ("2023-03-01", "0.5617283945"),
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
                ("2022-01-01", "3.5576131651666665"),
                ("2023-01-01", "6.600308635375001"),
            ],
        )
        .await;
    }
}
