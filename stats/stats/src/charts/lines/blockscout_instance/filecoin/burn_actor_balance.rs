// SPDX-License-Identifier: LicenseRef-Blockscout

//! Daily balance of the Filecoin burn actor (f099), in FIL.
//!
//! The burn actor accumulates the base-fee burn and the over-estimation
//! burn of every message on the chain, so its balance is a cumulative
//! series of the burnt part of chain fees. `Delta` over this chart (see
//! `filecoin_new_chain_fees`) recovers the per-day burn.
//!
//! Internal-only chart: never exposed through the API (stays disabled),
//! updated transitively as a dependency of the public Filecoin charts.

use std::{collections::HashSet, ops::Range};

use crate::{chart_prelude::*, utils::ETHER};

/// Hex of the 20-byte EVM representation of the Filecoin burn actor (f099).
pub const BURN_ACTOR_HASH_HEX: &str = "ff00000000000000000000000000000000000063";

pub struct BurnActorBalanceStatement;
impl_db_choice!(BurnActorBalanceStatement, UsePrimaryDB);

impl StatementFromRange for BurnActorBalanceStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        let day_range: Option<Range<NaiveDate>> = range.map(|r| {
            let Range { start, end } = r;
            start.date_naive()..end.date_naive()
        });
        // query uses a date column, therefore `sql_with_range_filter_opt`
        // does not quite fit (it compares against timestamps, and its
        // half-open end would drop the end day whenever a batch seam lands
        // on midnight) — the closed-interval date helper is used instead
        let mut values = vec![ETHER.into(), BURN_ACTOR_HASH_HEX.into()];
        let (day_filter, day_values) =
            produce_day_filter_and_values(day_range, "day", values.len() + 1);
        values.extend(day_values);
        let sql = format!(
            r"
                SELECT
                    day as date,
                    (value / $1)::float AS value
                FROM address_coin_balances_daily
                WHERE
                    address_hash = decode($2, 'hex') AND
                    value is not NULL AND
                    day != to_timestamp(0){day_filter};
            "
        );
        Statement::from_sql_and_values(DbBackend::Postgres, sql, values)
    }
}

pub type BurnActorBalanceRemote = RemoteDatabaseSource<
    PullAllWithAndSort<BurnActorBalanceStatement, NaiveDate, f64, QueryFullIndexerTimestampRange>,
>;

pub type BurnActorBalanceRemoteString = MapToString<BurnActorBalanceRemote>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "burnActorBalance".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        // cumulative balance carries over the days without changes;
        // required for correct `Delta` over this chart
        MissingDatePolicy::FillPrevious
    }
}

// `BatchMaxDays`: a light single-address lookup on the
// `(address_hash, day)` primary key returning one tiny row per day — the
// whole range is cheaper in one query (cf. `NewAccounts`), unlike the heavy
// join of `FevmFeeTips`, which stays batched
pub type BurnActorBalance =
    DirectVecLocalDbChartSource<BurnActorBalanceRemoteString, BatchMaxDays, Properties>;
pub type BurnActorBalanceFloat = MapParseTo<StripExt<BurnActorBalance>, f64>;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tests::{
        normalize_sql, point_construction::dt, simple_test::simple_test_chart_filecoin,
    };

    #[test]
    fn burn_actor_hash_is_20_bytes() {
        assert_eq!(BURN_ACTOR_HASH_HEX.len(), 40);
        assert_eq!(hex::decode(BURN_ACTOR_HASH_HEX).map(|v| v.len()), Ok(20));
    }

    #[test]
    fn statement_with_range_is_correct() {
        let actual = BurnActorBalanceStatement::get_statement(
            Some(dt("2023-01-01T00:00:00").and_utc()..dt("2023-02-01T00:00:00").and_utc()),
            &IndexerMigrations::latest(),
            &HashSet::new(),
        );
        let expected = r"
            SELECT
                day as date,
                (value / 1000000000000000000)::float AS value
            FROM address_coin_balances_daily
            WHERE
                address_hash = decode('ff00000000000000000000000000000000000063', 'hex') AND
                value is not NULL AND
                day != to_timestamp(0) AND
                day >= '2023-01-01' AND
                day <= '2023-02-01';
        ";
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()));
    }

    #[test]
    fn statement_without_range_is_correct() {
        let actual = BurnActorBalanceStatement::get_statement(
            None,
            &IndexerMigrations::latest(),
            &HashSet::new(),
        );
        let expected = r"
            SELECT
                day as date,
                (value / 1000000000000000000)::float AS value
            FROM address_coin_balances_daily
            WHERE
                address_hash = decode('ff00000000000000000000000000000000000063', 'hex') AND
                value is not NULL AND
                day != to_timestamp(0);
        ";
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_burn_actor_balance() {
        simple_test_chart_filecoin::<BurnActorBalance>(
            "update_burn_actor_balance",
            vec![
                ("2022-11-09", "30000000"),
                ("2022-11-10", "30001000"),
                ("2022-11-12", "30003500"),
                ("2022-12-01", "30010000"),
                ("2023-01-01", "30020000"),
                ("2023-02-01", "30035000"),
                ("2023-03-01", "30050000"),
            ],
        )
        .await;
    }
}
