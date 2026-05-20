use std::{collections::HashSet, ops::Range};

use crate::{
    chart_prelude::*,
    lines::{NewBlocksInt, NewBlocksMonthlyInt},
};

const GWEI: i64 = 1_000_000_000;

pub struct AverageGasPriceStatement;
impl_db_choice!(AverageGasPriceStatement, UsePrimaryDB);

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
            let (user_block_filter, new_args) =
                produce_filter_and_values(range.clone(), "b.timestamp", args.len() + 1);
            args.extend(new_args);
            let (fallback_block_filter, new_args) =
                produce_filter_and_values(range.clone(), "b.timestamp", args.len() + 1);
            args.extend(new_args);
            let sql = format!(
                r#"

                    WITH per_day_user_avg AS (
                        SELECT b.timestamp::date AS date,
                               AVG(COALESCE(
                                   t_filtered.gas_price,
                                   b.base_fee_per_gas + LEAST(
                                       t_filtered.max_priority_fee_per_gas,
                                       t_filtered.max_fee_per_gas - b.base_fee_per_gas
                                   )
                               )) AS user_price
                        FROM (
                            SELECT * from transactions t
                            WHERE
                                t.block_consensus = true AND
                                t.block_timestamp != to_timestamp(0) AND
                                -- Exclude OP-stack deposit (system) transactions (type 0x7E = 126):
                                -- they are funded on L1 and stored with gas_price=0, which would
                                -- otherwise drag the daily average toward zero. IS DISTINCT FROM
                                -- keeps pre-Berlin NULL-typed legacy txs.
                                t.type IS DISTINCT FROM 126 {tx_filter}
                        ) as t_filtered
                        JOIN blocks b ON t_filtered.block_hash = b.hash
                        WHERE
                            b.timestamp != to_timestamp(0) AND
                            b.consensus = true {user_block_filter}
                        GROUP BY date
                    ),
                    per_day_block_avg AS (
                        -- Fallback for days that have blocks but no user (non-deposit) txs.
                        -- Reports the mean EIP-1559 base fee for those days so the chart shows
                        -- the chain's floor instead of a gap.
                        SELECT b.timestamp::date AS date,
                               AVG(b.base_fee_per_gas) AS block_price
                        FROM blocks b
                        WHERE
                            b.consensus = true AND
                            b.timestamp != to_timestamp(0) AND
                            b.base_fee_per_gas IS NOT NULL {fallback_block_filter}
                        GROUP BY date
                    )
                    SELECT COALESCE(ud.date, bd.date) AS date,
                           (COALESCE(ud.user_price, bd.block_price) / $1)::float AS value
                    FROM per_day_user_avg ud
                    FULL OUTER JOIN per_day_block_avg bd ON ud.date = bd.date
                    WHERE COALESCE(ud.user_price, bd.block_price) IS NOT NULL
                "#,
            );
            Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
        } else {
            let mut args = vec![GWEI.into()];
            let (user_block_filter, new_args) =
                produce_filter_and_values(range.clone(), "blocks.timestamp", args.len() + 1);
            args.extend(new_args);
            let (fallback_block_filter, new_args) =
                produce_filter_and_values(range.clone(), "blocks.timestamp", args.len() + 1);
            args.extend(new_args);
            let sql = format!(
                r#"
                    WITH per_day_user_avg AS (
                        SELECT blocks.timestamp::date AS date,
                               AVG(COALESCE(
                                   transactions.gas_price,
                                   blocks.base_fee_per_gas + LEAST(
                                       transactions.max_priority_fee_per_gas,
                                       transactions.max_fee_per_gas - blocks.base_fee_per_gas
                                   )
                               )) AS user_price
                        FROM transactions
                        JOIN blocks ON transactions.block_hash = blocks.hash
                        WHERE
                            blocks.timestamp != to_timestamp(0) AND
                            blocks.consensus = true AND
                            -- Exclude OP-stack deposit (system) txs; see denormalized branch
                            -- above for rationale.
                            transactions.type IS DISTINCT FROM 126 {user_block_filter}
                        GROUP BY date
                    ),
                    per_day_block_avg AS (
                        -- Fallback: mean EIP-1559 base fee for days with blocks but no user
                        -- (non-deposit) txs.
                        SELECT blocks.timestamp::date AS date,
                               AVG(blocks.base_fee_per_gas) AS block_price
                        FROM blocks
                        WHERE
                            blocks.consensus = true AND
                            blocks.timestamp != to_timestamp(0) AND
                            blocks.base_fee_per_gas IS NOT NULL {fallback_block_filter}
                        GROUP BY date
                    )
                    SELECT COALESCE(ud.date, bd.date) AS date,
                           (COALESCE(ud.user_price, bd.block_price) / $1)::float AS value
                    FROM per_day_user_avg ud
                    FULL OUTER JOIN per_day_block_avg bd ON ud.date = bd.date
                    WHERE COALESCE(ud.user_price, bd.block_price) IS NOT NULL
                "#,
            );
            Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
        }
    }
}

pub type AverageGasPriceRemote = RemoteDatabaseSource<
    PullAllWithAndSort<AverageGasPriceStatement, NaiveDate, f64, QueryFullIndexerTimestampRange>,
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
// Low-res averageGasPrice is block-weighted (NewBlocksInt / NewBlocksMonthlyInt) because the
// daily value may fall back to AVG(blocks.base_fee_per_gas) on days with no user (non-deposit)
// transactions. Weighting by total tx count would (a) overweight deposit-heavy days, since
// deposits inflate tx count but contribute nothing to the daily average, and (b) zero out the
// weight of fallback-only days, dropping them from the lower-resolution average entirely.
// Block count is non-zero whenever the daily series has a value and is independent of
// deposit volume, so it cleanly propagates the daily semantics.
pub type AverageGasPriceWeekly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageGasPriceS, f64>, NewBlocksInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AverageGasPriceMonthly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageGasPriceS, f64>, NewBlocksInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
type AverageGasPriceMonthlyS = StripExt<AverageGasPriceMonthly>;
pub type AverageGasPriceYearly = DirectVecLocalDbChartSource<
    MapToString<
        AverageLowerResolution<MapParseTo<AverageGasPriceMonthlyS, f64>, NewBlocksMonthlyInt, Year>,
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

    // Low-res averageGasPrice is block-weighted (see `AverageGasPriceWeekly` type alias above
    // for rationale). The week-of-2022-11-07 / month-of-2022-11 values below sit between the
    // daily extremes, weighted by the mock fixture's 1/3/4/1 block-per-day distribution across
    // Nov 9-12, and differ from the prior tx-weighted expectations.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_price_weekly() {
        simple_test_chart_with_migration_variants::<AverageGasPriceWeekly>(
            "update_average_gas_price_weekly",
            vec![
                ("2022-11-07", "3.0672153604444445"),
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
                ("2022-11-01", "3.0672153604444445"),
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
                ("2022-01-01", "3.472016457433333"),
                ("2023-01-01", "3.9320987615000003"),
            ],
        )
        .await;
    }
}
