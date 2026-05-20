//! Average fee per transaction

use std::{collections::HashSet, ops::Range};

use crate::{
    chart_prelude::*,
    lines::{NewBlocksInt, NewBlocksMonthlyInt},
};

const ETHER: i64 = i64::pow(10, 18);
/// Gas used by a plain native-coin transfer. Hard-coded floor used by the
/// empty-user-tx fallback so the chart shows what a simple tx would have cost
/// at the day's mean base fee, rather than a gap. Contract calls cost more in
/// practice, but this gives a stable, chain-agnostic lower bound — matching
/// the way wallets quote a "send" fee.
const BASIC_TX_GAS: i64 = 21_000;

pub struct AverageTxnFeeStatement;
impl_db_choice!(AverageTxnFeeStatement, UsePrimaryDB);

impl StatementFromRange for AverageTxnFeeStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        if completed_migrations.denormalization {
            // TODO: consider supporting such case in macro ?
            let mut args = vec![ETHER.into(), BASIC_TX_GAS.into()];
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
                        SELECT DATE(b.timestamp) AS date,
                               AVG(
                                   t_filtered.gas_used *
                                   COALESCE(
                                       t_filtered.gas_price,
                                       b.base_fee_per_gas + LEAST(
                                           t_filtered.max_priority_fee_per_gas,
                                           t_filtered.max_fee_per_gas - b.base_fee_per_gas
                                       )
                                   )
                               ) AS user_fee_wei
                        FROM (
                            SELECT * from transactions t
                            WHERE
                                t.block_consensus = true AND
                                t.block_timestamp != to_timestamp(0) AND
                                -- Exclude OP-stack deposit (system) transactions (type 0x7E = 126):
                                -- they are funded on L1 and stored with gas_price=0, so their fee
                                -- would otherwise drag the daily average toward zero. IS DISTINCT
                                -- FROM keeps pre-Berlin NULL-typed legacy txs.
                                t.type IS DISTINCT FROM 126 {tx_filter}
                        ) as t_filtered
                        JOIN blocks b ON t_filtered.block_hash = b.hash
                        WHERE
                            b.timestamp != to_timestamp(0) AND
                            b.consensus = true {user_block_filter}
                        GROUP BY DATE(b.timestamp)
                    ),
                    per_day_block_avg AS (
                        -- Fallback for days that have blocks but no user (non-deposit) txs.
                        -- Estimates the fee as BASIC_TX_GAS ($2 = 21000, a plain native-coin
                        -- transfer) at the day's mean EIP-1559 base fee, so the chart shows the
                        -- chain's floor instead of a gap on idle days.
                        SELECT DATE(b.timestamp) AS date,
                               $2 * AVG(b.base_fee_per_gas) AS block_fee_wei
                        FROM blocks b
                        WHERE
                            b.consensus = true AND
                            b.timestamp != to_timestamp(0) AND
                            b.base_fee_per_gas IS NOT NULL {fallback_block_filter}
                        GROUP BY DATE(b.timestamp)
                    )
                    SELECT COALESCE(ud.date, bd.date) AS date,
                           (COALESCE(ud.user_fee_wei, bd.block_fee_wei) / $1)::FLOAT AS value
                    FROM per_day_user_avg ud
                    FULL OUTER JOIN per_day_block_avg bd ON ud.date = bd.date
                    WHERE COALESCE(ud.user_fee_wei, bd.block_fee_wei) IS NOT NULL
                "#,
            );
            Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
        } else {
            let mut args = vec![ETHER.into(), BASIC_TX_GAS.into()];
            let (user_block_filter, new_args) =
                produce_filter_and_values(range.clone(), "b.timestamp", args.len() + 1);
            args.extend(new_args);
            let (fallback_block_filter, new_args) =
                produce_filter_and_values(range.clone(), "b.timestamp", args.len() + 1);
            args.extend(new_args);
            let sql = format!(
                r#"
                    WITH per_day_user_avg AS (
                        SELECT DATE(b.timestamp) AS date,
                               AVG(
                                   t.gas_used *
                                   COALESCE(
                                       t.gas_price,
                                       b.base_fee_per_gas + LEAST(
                                           t.max_priority_fee_per_gas,
                                           t.max_fee_per_gas - b.base_fee_per_gas
                                       )
                                   )
                               ) AS user_fee_wei
                        FROM transactions t
                        JOIN blocks b ON t.block_hash = b.hash
                        WHERE
                            b.timestamp != to_timestamp(0) AND
                            b.consensus = true AND
                            -- Exclude OP-stack deposit (system) txs; see denormalized branch
                            -- above for rationale.
                            t.type IS DISTINCT FROM 126 {user_block_filter}
                        GROUP BY DATE(b.timestamp)
                    ),
                    per_day_block_avg AS (
                        -- Fallback: BASIC_TX_GAS ($2 = 21000) at the day's mean base fee.
                        SELECT DATE(b.timestamp) AS date,
                               $2 * AVG(b.base_fee_per_gas) AS block_fee_wei
                        FROM blocks b
                        WHERE
                            b.consensus = true AND
                            b.timestamp != to_timestamp(0) AND
                            b.base_fee_per_gas IS NOT NULL {fallback_block_filter}
                        GROUP BY DATE(b.timestamp)
                    )
                    SELECT COALESCE(ud.date, bd.date) AS date,
                           (COALESCE(ud.user_fee_wei, bd.block_fee_wei) / $1)::FLOAT AS value
                    FROM per_day_user_avg ud
                    FULL OUTER JOIN per_day_block_avg bd ON ud.date = bd.date
                    WHERE COALESCE(ud.user_fee_wei, bd.block_fee_wei) IS NOT NULL
                "#,
            );
            Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
        }
    }
}

pub type AverageTxnFeeRemote = RemoteDatabaseSource<
    PullAllWithAndSort<AverageTxnFeeStatement, NaiveDate, f64, QueryFullIndexerTimestampRange>,
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
// Low-res averageTxnFee is block-weighted (NewBlocksInt / NewBlocksMonthlyInt) for the same
// reason as averageGasPrice: the daily value may fall back to BASIC_TX_GAS * AVG(base_fee) on
// days with no user (non-deposit) txs. Weighting by total tx count would overweight
// deposit-heavy days and ignore fallback-only days. Block count is non-zero whenever the
// daily series has a value and is independent of deposit volume.
pub type AverageTxnFeeWeekly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageTxnFeeS, f64>, NewBlocksInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AverageTxnFeeMonthly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageTxnFeeS, f64>, NewBlocksInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
type AverageTxnFeeMonthlyS = StripExt<AverageTxnFeeMonthly>;
pub type AverageTxnFeeYearly = DirectVecLocalDbChartSource<
    MapToString<
        AverageLowerResolution<MapParseTo<AverageTxnFeeMonthlyS, f64>, NewBlocksMonthlyInt, Year>,
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

    // Low-res averageTxnFee is block-weighted (see `AverageTxnFeeWeekly` type alias above
    // for rationale). The week-of-2022-11-07 / month-of-2022-11 values below reflect the
    // mock fixture's 1/3/4/1 block-per-day distribution across Nov 9-12.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txn_fee_weekly() {
        simple_test_chart_with_migration_variants::<AverageTxnFeeWeekly>(
            "update_average_txn_fee_weekly",
            vec![
                ("2022-11-07", "0.00006441152256933334"),
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
                ("2022-11-01", "0.00006441152256933334"),
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
                ("2022-01-01", "0.0000729123456061"),
                ("2023-01-01", "0.0000825740739915"),
            ],
        )
        .await;
    }
}
