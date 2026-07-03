// SPDX-License-Identifier: LicenseRef-Blockscout

//! Per-day sum of FEVM miner tips, in FIL.
//!
//! A miner tip is `(gas_price - base_fee_per_gas) * gas_used`; on Filecoin
//! `gas_price` is always populated for FEVM transactions, so the literal
//! subtraction is correct. Together with the per-day burn delta (see
//! `burn_actor_balance`) it forms the chain-wide fees total.
//!
//! Internal-only chart: never exposed through the API (stays disabled),
//! updated transitively as a dependency of the public Filecoin charts.

use std::{collections::HashSet, ops::Range};

use crate::chart_prelude::*;

const ETHER: i64 = i64::pow(10, 18);

pub struct FevmFeeTipsStatement;
impl_db_choice!(FevmFeeTipsStatement, UsePrimaryDB);

impl StatementFromRange for FevmFeeTipsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        // the outer `value IS NOT NULL` guard drops days where every tip
        // term is NULL (e.g. all blocks of the day lack `base_fee_per_gas`);
        // without it a NULL `SUM` fails deserialization and takes the whole
        // update down
        if completed_migrations.denormalization {
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
                                (t_filtered.gas_price - b.base_fee_per_gas) * t_filtered.gas_used
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
                                (t.gas_price - b.base_fee_per_gas) * t.gas_used
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
        normalize_sql, point_construction::dt,
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
                        (t_filtered.gas_price - b.base_fee_per_gas) * t_filtered.gas_used
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
                        (t.gas_price - b.base_fee_per_gas) * t.gas_used
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
        simple_test_chart_filecoin_with_migration_variants::<FevmFeeTips>(
            "update_fevm_fee_tips",
            vec![
                ("2022-11-10", "0.000584007406794"),
                ("2022-11-11", "0.001193214813588"),
                ("2022-11-12", "0.000789548147346"),
                ("2022-12-01", "0.000883918517622"),
                ("2023-01-01", "0.000021492592569"),
                ("2023-02-01", "0.001051166665605"),
            ],
        )
        .await;
    }
}
