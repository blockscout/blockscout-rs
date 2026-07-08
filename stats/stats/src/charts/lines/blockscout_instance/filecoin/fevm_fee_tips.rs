// SPDX-License-Identifier: LicenseRef-Blockscout

//! Per-day sum of FEVM miner tips, in FIL.
//!
//! A miner tip is `(gas_price - base_fee_per_gas) * gas_used`, floored at
//! zero per transaction. Filecoin permits a message with
//! `gas_fee_cap < base_fee` — the miner absorbs the shortfall as a penalty
//! (burned into f099, i.e. already on the burn side of the chain-fees
//! total) and the protocol clamps that message's tip to exactly 0. The
//! query enforces the same floor instead of assuming
//! `gas_price >= base_fee_per_gas` always holds in indexer data.
//! Together with the per-day burn delta (see `burn_actor_balance`) it
//! forms the chain-wide fees total.
//!
//! Internal-only chart: never exposed through the API (stays disabled),
//! updated transitively as a dependency of the public Filecoin charts.

use std::{collections::HashSet, ops::Range};

use crate::{chart_prelude::*, utils::ETHER};

pub struct FevmFeeTipsStatement;
impl_db_choice!(FevmFeeTipsStatement, UsePrimaryDB);

impl StatementFromRange for FevmFeeTipsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        // the tip floor is a `CASE` rather than `GREATEST(diff, 0)` so that
        // a NULL `base_fee_per_gas` keeps the term NULL (`GREATEST` ignores
        // NULL arguments and would coerce it to 0). The outer
        // `value IS NOT NULL` guard then drops days where every tip term is
        // NULL (e.g. all blocks of the day lack `base_fee_per_gas`);
        // without it a NULL `SUM` fails deserialization and takes the whole
        // update down. On "mixed" days (only some blocks lack the base fee)
        // the NULL terms are skipped by `SUM` and the day passes the guard
        // with a deliberately-accepted understated sum — closer to the truth
        // (and to the expected shape of the cumulative chart) than a
        // fabricated zero-fee day; see
        // `mixed_day_value_characterizes_the_undercount`
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
                    SELECT date, value FROM (
                        SELECT
                            DATE(b.timestamp) as date,
                            (SUM(
                                CASE
                                    WHEN t_filtered.gas_price < b.base_fee_per_gas THEN 0
                                    ELSE (t_filtered.gas_price - b.base_fee_per_gas) * t_filtered.gas_used
                                END
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
                    ) as intermediate
                    WHERE value is not NULL
                "#,
            );
            Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
        } else {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT date, value FROM (
                        SELECT
                            DATE(b.timestamp) as date,
                            (SUM(
                                CASE
                                    WHEN t.gas_price < b.base_fee_per_gas THEN 0
                                    ELSE (t.gas_price - b.base_fee_per_gas) * t.gas_used
                                END
                            ) / $1)::FLOAT as value
                        FROM transactions t
                        JOIN blocks       b ON t.block_hash = b.hash
                        WHERE
                            b.timestamp != to_timestamp(0) AND
                            b.consensus = true {filter}
                        GROUP BY DATE(b.timestamp)
                    ) as intermediate
                    WHERE value is not NULL
                "#,
                [ETHER.into()],
                "b.timestamp",
                range
            )
        }
    }
}

pub type FevmFeeTipsRemote = RemoteDatabaseSource<
    PullAllWithAndSort<FevmFeeTipsStatement, NaiveDate, f64, QueryFullIndexerTimestampRange>,
>;

pub type FevmFeeTipsRemoteString = MapToString<FevmFeeTipsRemote>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "fevmFeeTips".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        // a day without FEVM transactions contributes 0 tips
        MissingDatePolicy::FillZero
    }
}

pub type FevmFeeTips =
    DirectVecLocalDbChartSource<FevmFeeTipsRemoteString, Batch30Days, Properties>;
pub type FevmFeeTipsFloat = MapParseTo<StripExt<FevmFeeTips>, f64>;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tests::{
        mock_blockscout_filecoin::{mixed_day_complete_tips_fil, mixed_day_counted_tips_fil},
        normalize_sql,
        point_construction::dt,
        simple_test::simple_test_chart_filecoin_with_migration_variants,
    };

    #[test]
    fn statement_denormalized_is_correct() {
        let actual = FevmFeeTipsStatement::get_statement(
            Some(dt("2023-01-01T00:00:00").and_utc()..dt("2023-02-01T00:00:00").and_utc()),
            &IndexerMigrations::latest(),
            &HashSet::new(),
        );
        let expected = r#"
            SELECT date, value FROM (
                SELECT
                    DATE(b.timestamp) as date,
                    (SUM(
                        CASE
                            WHEN t_filtered.gas_price < b.base_fee_per_gas THEN 0
                            ELSE (t_filtered.gas_price - b.base_fee_per_gas) * t_filtered.gas_used
                        END
                    ) / 1000000000000000000)::FLOAT as value
                FROM (
                    SELECT * from transactions t
                    WHERE
                        t.block_consensus = true AND
                        t.block_timestamp != to_timestamp(0) AND
                        t.block_timestamp < '2023-02-01 00:00:00.000000 +00:00' AND
                        t.block_timestamp >= '2023-01-01 00:00:00.000000 +00:00'
                ) as t_filtered
                JOIN blocks       b ON t_filtered.block_hash = b.hash
                WHERE
                    b.timestamp != to_timestamp(0) AND
                    b.consensus = true AND
                    b.timestamp < '2023-02-01 00:00:00.000000 +00:00' AND
                    b.timestamp >= '2023-01-01 00:00:00.000000 +00:00'
                GROUP BY DATE(b.timestamp)
            ) as intermediate
            WHERE value is not NULL
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()));
    }

    #[test]
    fn statement_non_denormalized_is_correct() {
        let actual = FevmFeeTipsStatement::get_statement(
            Some(dt("2023-01-01T00:00:00").and_utc()..dt("2023-02-01T00:00:00").and_utc()),
            &IndexerMigrations::empty(),
            &HashSet::new(),
        );
        let expected = r#"
            SELECT date, value FROM (
                SELECT
                    DATE(b.timestamp) as date,
                    (SUM(
                        CASE
                            WHEN t.gas_price < b.base_fee_per_gas THEN 0
                            ELSE (t.gas_price - b.base_fee_per_gas) * t.gas_used
                        END
                    ) / 1000000000000000000)::FLOAT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE
                    b.timestamp != to_timestamp(0) AND
                    b.consensus = true AND
                    b.timestamp < '2023-02-01 00:00:00.000000 +00:00' AND
                    b.timestamp >= '2023-01-01 00:00:00.000000 +00:00'
                GROUP BY DATE(b.timestamp)
            ) as intermediate
            WHERE value is not NULL
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_fevm_fee_tips() {
        // `2022-11-09` and `2023-03-01` are deliberately omitted: the only
        // block of each day carries a zero-gas-price transaction, so it keeps
        // `base_fee_per_gas = NULL` (fixture hazard rule), the day's `SUM` is
        // NULL and the row is dropped by the outer `value IS NOT NULL` guard.
        // `2023-02-14` is the "mixed" day: its hazard block's tip terms are
        // NULL and skipped by `SUM`, so the stored value is understated (see
        // `mixed_day_value_characterizes_the_undercount`).
        simple_test_chart_filecoin_with_migration_variants::<FevmFeeTips>(
            "update_fevm_fee_tips",
            vec![
                ("2022-11-10", "0.000584007406794"),
                ("2022-11-11", "0.001193214813588"),
                ("2022-11-12", "0.000789548147346"),
                ("2022-12-01", "0.000883918517622"),
                ("2023-01-01", "0.000021492592569"),
                ("2023-02-01", "0.001051166665605"),
                ("2023-02-14", "0.0001"),
            ],
        )
        .await;
    }

    /// Characterizes the current, deliberately-accepted behavior on "mixed"
    /// days (some blocks of the day carry `base_fee_per_gas`, some are NULL):
    /// the NULL blocks' transactions produce NULL tip terms, `SUM` skips
    /// them, and the day passes the `value IS NOT NULL` guard with an
    /// understated sum (PR #1695 review decision).
    ///
    /// This is a known limitation, not the target: the root cause
    /// (source-data completeness) is owned by the Blockscout instance, and a
    /// dropped day would be a *more* misleading failure mode than a slight
    /// understatement. Detection/alerting of partially-populated days is
    /// tracked separately (`.ai/issues/`, partial-day detection). Anyone
    /// changing the guard semantics must consciously update this test and
    /// the `2023-02-14` expectation in `update_fevm_fee_tips`.
    #[test]
    fn mixed_day_value_characterizes_the_undercount() {
        let counted = mixed_day_counted_tips_fil();
        let complete = mixed_day_complete_tips_fil();
        assert!(
            counted < complete,
            "the mixed day must lose some tips ({counted} vs {complete})"
        );
        // the understated value is exactly what `update_fevm_fee_tips` pins
        // for `2023-02-14`; the complete-data value is what the day would be
        // if every block carried its base fee
        assert_eq!(counted.to_string(), "0.0001");
        assert_eq!(complete.to_string(), "0.0004");
    }
}
