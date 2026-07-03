// SPDX-License-Identifier: LicenseRef-Blockscout

//! An isolated Filecoin-specific layer on top of the shared
//! [`mock_blockscout`](super::mock_blockscout) fixture.
//!
//! Provides the data shapes required by the Filecoin chain-fees charts:
//! - `address_coin_balances_daily` rows for the f099 burn actor;
//! - `base_fee_per_gas` values on consensus blocks (the shared fixture
//!   leaves the column NULL everywhere).
//!
//! The layer is applied as an *additional* fill step so the data seen by
//! every existing test stays byte-for-byte unchanged; only tests that opt
//! in via `simple_test_chart_filecoin*` (or call
//! [`fill_mock_blockscout_filecoin_data`] directly) observe it.

#![cfg(any(feature = "test-utils", test))]

use blockscout_db::entity::address_coin_balances_daily;
use chrono::NaiveDate;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, Statement, prelude::Decimal,
};
use std::str::FromStr;

use super::mock_blockscout::mock_address_coin_balance_daily;

/// Hex of the 20-byte EVM representation of the Filecoin burn actor (f099).
///
/// Must stay in sync with `BURN_ACTOR_HASH_HEX` in
/// `crate::charts::lines::blockscout_instance::filecoin::burn_actor_balance`.
pub const BURN_ACTOR_HASH_HEX: &str = "ff00000000000000000000000000000000000063";

const ETHER: i128 = i128::pow(10, 18);

/// Deterministic base fee set on non-hazard consensus blocks.
///
/// Strictly below the minimum non-zero `gas_price` in the shared fixture
/// (`1_123_456_789`), so every counted miner tip
/// `(gas_price - base_fee_per_gas) * gas_used` is positive and
/// hand-computable.
const BASE_FEE_PER_GAS: i64 = 100_000_000;

/// Burn-actor (f099) balances per day, in whole FIL.
///
/// Monotonically increasing and deliberately sparse:
/// - `2022-11-11` has no row while FEVM transactions exist that day —
///   a "tips-only" day exercising the `FillPrevious` carry-forward;
/// - `2022-12-15` has neither an f099 row nor any block — the genuine
///   no-data day asserted by absence at chart level and by filled values
///   at the API level;
/// - `2023-03-01` has a row while its only block keeps
///   `base_fee_per_gas = NULL` (see the hazard rule below) — a natural
///   "burn-only" day.
fn burn_actor_balances_fil() -> Vec<(NaiveDate, i128)> {
    [
        ("2022-11-09", 30_000_000),
        ("2022-11-10", 30_001_000),
        ("2022-11-12", 30_003_500),
        ("2022-12-01", 30_010_000),
        ("2023-01-01", 30_020_000),
        ("2023-02-01", 30_035_000),
        ("2023-03-01", 30_050_000),
    ]
    .into_iter()
    .map(|(day, fil)| (NaiveDate::from_str(day).unwrap(), fil))
    .collect()
}

fn mock_burn_actor_balances(max_date: NaiveDate) -> Vec<address_coin_balances_daily::ActiveModel> {
    let burn_actor_hash = hex::decode(BURN_ACTOR_HASH_HEX).unwrap();
    assert_eq!(burn_actor_hash.len(), 20, "f099 hash must be 20 bytes");
    burn_actor_balances_fil()
        .into_iter()
        .filter(|(day, _)| *day <= max_date)
        .map(|(day, fil)| {
            mock_address_coin_balance_daily(burn_actor_hash.clone(), day, Some(fil * ETHER))
        })
        .collect()
}

/// Fills Filecoin-specific data on top of the shared fixture
/// ([`super::mock_blockscout::fill_mock_blockscout_data`], which must have
/// been applied already with the same `max_date`):
///
/// - inserts f099 `address_coin_balances_daily` rows
///   (see [`burn_actor_balances_fil`]);
/// - sets [`BASE_FEE_PER_GAS`] on every consensus block that carries
///   transactions, **except** blocks carrying at least one
///   `gas_price = 0` transaction ("hazard" blocks, which keep NULL).
///
/// The hazard rule mirrors production Filecoin, where the effective gas
/// price never falls below the base fee, so miner tips are non-negative:
/// a positive base fee under a zero-priced fixture transaction would
/// produce a negative tip. Hazard blocks (currently block 0 on
/// `2022-11-09` and block 12 on `2023-03-01`, each the only block of its
/// day) leave all tip terms of their day NULL, so the day is dropped by
/// the `value IS NOT NULL` guard of `fevmFeeTips`.
pub async fn fill_mock_blockscout_filecoin_data(
    blockscout: &DatabaseConnection,
    max_date: NaiveDate,
) {
    address_coin_balances_daily::Entity::insert_many(mock_burn_actor_balances(max_date))
        .exec(blockscout)
        .await
        .unwrap();

    blockscout
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
                UPDATE blocks AS b
                SET base_fee_per_gas = $1
                WHERE
                    b.consensus = true AND
                    EXISTS (
                        SELECT 1 FROM transactions t
                        WHERE t.block_hash = b.hash
                    ) AND
                    NOT EXISTS (
                        SELECT 1 FROM transactions t
                        WHERE t.block_hash = b.hash AND t.gas_price = 0
                    )
            "#,
            vec![Decimal::from(BASE_FEE_PER_GAS).into()],
        ))
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tests::{init_db::init_db_blockscout, mock_blockscout::fill_mock_blockscout_data};

    #[test]
    fn burn_actor_hash_is_20_bytes() {
        assert_eq!(BURN_ACTOR_HASH_HEX.len(), 40);
        assert_eq!(hex::decode(BURN_ACTOR_HASH_HEX).map(|v| v.len()), Ok(20));
    }

    #[test]
    fn burn_actor_balances_are_consistent() {
        let balances = burn_actor_balances_fil();
        assert!(!balances.is_empty());
        let unique_days: HashSet<_> = balances.iter().map(|(day, _)| *day).collect();
        assert_eq!(
            unique_days.len(),
            balances.len(),
            "duplicate (address, day)"
        );
        assert!(
            balances
                .windows(2)
                .all(|w| w[0].1 < w[1].1 && w[0].0 < w[1].0),
            "balances must be sorted and monotonically increasing"
        );
        // the genuine no-data day must stay uncovered
        let no_data_day = NaiveDate::from_str("2022-12-15").unwrap();
        assert!(!unique_days.contains(&no_data_day));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn base_fee_rule_holds() {
        let blockscout = init_db_blockscout("mock_blockscout_filecoin_base_fee_rule").await;
        let max_date = NaiveDate::from_str("2023-03-01").unwrap();
        fill_mock_blockscout_data(&blockscout, max_date).await;
        fill_mock_blockscout_filecoin_data(&blockscout, max_date).await;

        let count = |sql: &str| {
            let statement = Statement::from_string(DbBackend::Postgres, sql.to_string());
            async {
                blockscout
                    .query_one(statement)
                    .await
                    .unwrap()
                    .unwrap()
                    .try_get_by::<i64, _>(0)
                    .unwrap()
            }
        };

        // no block carrying a zero-gas-price transaction has a base fee
        let violating_blocks = count(
            "SELECT COUNT(*) FROM blocks b \
            WHERE b.base_fee_per_gas IS NOT NULL AND EXISTS ( \
                SELECT 1 FROM transactions t \
                WHERE t.block_hash = b.hash AND t.gas_price = 0 \
            )",
        )
        .await;
        assert_eq!(violating_blocks, 0);

        // the rule did set the base fee somewhere
        let blocks_with_base_fee =
            count("SELECT COUNT(*) FROM blocks WHERE base_fee_per_gas IS NOT NULL").await;
        assert!(blocks_with_base_fee > 0);

        // non-consensus blocks are untouched
        let non_consensus_with_base_fee = count(
            "SELECT COUNT(*) FROM blocks \
            WHERE base_fee_per_gas IS NOT NULL AND consensus = false",
        )
        .await;
        assert_eq!(non_consensus_with_base_fee, 0);

        // f099 rows are present
        let f099_rows = count(
            "SELECT COUNT(*) FROM address_coin_balances_daily \
            WHERE address_hash = decode('ff00000000000000000000000000000000000063', 'hex')",
        )
        .await;
        assert!(f099_rows > 0);
    }
}
