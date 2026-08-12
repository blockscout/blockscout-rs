// SPDX-License-Identifier: LicenseRef-Blockscout

//! An isolated Filecoin-specific layer on top of the shared
//! [`mock_blockscout`](super::mock_blockscout) fixture.
//!
//! Provides the data shapes required by the Filecoin chain-fees charts and
//! the 24-hour chain-fees counter:
//! - `address_coin_balances_daily` rows for the f099 burn actor;
//! - `base_fee_per_gas` values on consensus blocks (the shared fixture
//!   leaves the column NULL everywhere);
//! - a "mixed" day ([`MIXED_DAY`]): one priced block plus one hazard
//!   (NULL-base-fee) block carrying a normally-priced transaction, so the
//!   deliberately-accepted understated-sum behavior of `fevmFeeTips` has
//!   coverage (see `mixed_day_value_characterizes_the_undercount`);
//! - per-block `address_coin_balances` rows for the f099 burn actor, plus a
//!   series of consensus blocks inside and around the 24-hour counter's
//!   default test window (`2023-02-28T11:50` .. `2023-03-01T12:00`,
//!   [`counter_window_fixture`]) — the counter is the first consumer of the
//!   per-block table in the service (decision record
//!   `test-fixture-for-address-coin-balances.md`).
//!
//! The layer is applied as an *additional* fill step so the data seen by
//! every existing test stays byte-for-byte unchanged; only tests that opt
//! in via `simple_test_chart_filecoin*` / `simple_test_counter_filecoin*`
//! (or call [`fill_mock_blockscout_filecoin_data`] directly) observe it.

#![cfg(any(feature = "test-utils", test))]

use blockscout_db::entity::{
    address_coin_balances, address_coin_balances_daily, blocks, transactions,
};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::{
    ActiveValue::Set, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, Statement,
    prelude::Decimal,
};
use std::str::FromStr;

use super::mock_blockscout::{
    TxType, mock_address_coin_balance_daily, mock_addresses, mock_block, mock_transaction,
};
use crate::{lines::BURN_ACTOR_HASH_HEX, utils::ETHER};

/// Deterministic base fee set on non-hazard consensus blocks.
///
/// Strictly below the minimum non-zero `gas_price` in the shared fixture
/// (`1_123_456_789`), so every counted miner tip
/// `(gas_price - base_fee_per_gas) * gas_used` is positive and
/// hand-computable.
const BASE_FEE_PER_GAS: i64 = 100_000_000;

/// The "mixed" day: a priced block next to a hazard (NULL-base-fee) block
/// that carries a normally-priced transaction, mirroring a partially
/// backfilled `base_fee_per_gas` in production. `fevmFeeTips` counts only
/// the priced block ([`mixed_day_counted_tips_fil`]) and silently loses the
/// hazard block's tips — the deliberately-accepted understated-sum behavior
/// characterized by `mixed_day_value_characterizes_the_undercount`.
pub const MIXED_DAY: &str = "2023-02-14";
/// Gas price of the counted and the lost mixed-day transactions.
const MIXED_DAY_GAS_PRICE: i64 = 2_100_000_000;
/// Gas used by the counted (priced-block) mixed-day transaction.
const MIXED_DAY_COUNTED_GAS_USED: i64 = 50_000;
/// Gas used by the lost (hazard-block) normally-priced transaction.
const MIXED_DAY_LOST_GAS_USED: i64 = 150_000;
/// Mixed-day block numbers, far outside the shared fixture's `0..=12`.
const MIXED_DAY_PRICED_BLOCK: i64 = 100;
const MIXED_DAY_HAZARD_BLOCK: i64 = 101;

/// Tips actually counted on [`MIXED_DAY`] (the priced block only), in FIL.
pub fn mixed_day_counted_tips_fil() -> f64 {
    ((MIXED_DAY_GAS_PRICE - BASE_FEE_PER_GAS) * MIXED_DAY_COUNTED_GAS_USED) as f64 / ETHER as f64
}

/// Tips [`MIXED_DAY`] would carry if every block had its base fee, in FIL.
///
/// The hazard block's zero-gas-price transaction would contribute exactly 0
/// under the per-transaction tip floor, so only its normally-priced
/// transaction adds to the counted value. Summed in integers before the one
/// division, mirroring the query (`SUM` in numeric, then `/ $1`).
pub fn mixed_day_complete_tips_fil() -> f64 {
    ((MIXED_DAY_GAS_PRICE - BASE_FEE_PER_GAS)
        * (MIXED_DAY_COUNTED_GAS_USED + MIXED_DAY_LOST_GAS_USED)) as f64
        / ETHER as f64
}

// --- 24-hour counter window fixture -----------------------------------
//
// The `simple_test_counter*` harness hard-codes `max_time =
// 2023-03-01T12:00:00Z`, and `update_time` defaults to it
// (`simple_test.rs:567`, `:696`), so the counter's default test window is
// `[2023-02-28T12:00, 2023-03-01T12:00)`. The block numbers below are chosen
// deliberately, not just placed "far outside" the shared fixture's `0..=12`:
// the anchor CTEs of the counter's SQL find their bound as
// `max(blocks.number)` *inside* a timestamp filter, so number order and
// time order must agree at every edge a test resolves a bound at. Block 12
// (`2023-03-01T10:00`) is newer than `MIXED_DAY` blocks 100/101
// (`2023-02-14`), so every new consensus block below is numbered **above
// 101** and increases with its timestamp; at least one of them
// ([`COUNTER_END_ANCHOR_BLOCK`]) sits in `(2023-03-01T10:00,
// 2023-03-01T12:00)`, so the newest block before the window's end is also
// the highest-numbered one. See [`BOUND_EDGES`] and
// `block_numbers_follow_time_at_counter_edges`, which prove this property at
// the edges tests actually resolve.

/// Number of the pre-window start-anchor block (`2023-02-28T11:50:00`, ten
/// minutes before the counter's default window opens at noon). This is a
/// requirement in its own right: `bound_from` is by construction *before*
/// the window, so in-window blocks alone cannot fix the start anchor.
/// Without this block the anchor would stay on block 101
/// (`2023-02-14T12:00`, [`MIXED_DAY_HAZARD_BLOCK`]) and the burn would span
/// 14 days instead of ~24 hours (decision record
/// `20260811-1040/finding-03`).
const COUNTER_START_ANCHOR_BLOCK: i64 = 200;
/// First of two consecutive, closely-spaced blocks (five minutes apart) —
/// the "dense" density of fixture property 3.
const COUNTER_DENSE_BLOCK_1: i64 = 201;
/// Second dense block; carries the window's only priced transaction on
/// `2023-02-28` besides [`COUNTER_DOMINANT_BLOCK`]'s day.
const COUNTER_DENSE_BLOCK_2: i64 = 202;
/// First `value IS NULL` row (fixture property 5), several hours after the
/// dense pair — start of the "sparse" density.
const COUNTER_SPARSE_NULL_BLOCK_1: i64 = 203;
/// Carries most of the window's burn delta (fixture property 4); its value
/// is also the last per-block reading of `2023-02-28`, so it must equal the
/// day's `address_coin_balances_daily` row (decision record
/// `20260811-1040/finding-03`).
const COUNTER_DOMINANT_BLOCK: i64 = 204;
/// Second `value IS NULL` row, on `2023-03-01`.
const COUNTER_SPARSE_NULL_BLOCK_2: i64 = 205;
/// The end-anchor block (`2023-03-01T11:50:00`), strictly inside the
/// window's upper edge — together with [`COUNTER_START_ANCHOR_BLOCK`] this
/// makes the burn interval and the tips interval genuinely differ (fixture
/// property 2).
const COUNTER_END_ANCHOR_BLOCK: i64 = 206;
/// Highest-numbered consensus block in the fixture, dated **exactly** at
/// the default counter window's upper edge (`2023-03-01T12:00:00`).
/// Carries one priced transaction and an f099 row equal to the existing
/// `("2023-03-01", 30_050_000)` daily value — the block is now the last
/// block of that day, so the daily/per-block consistency rule forces the
/// last *in-window* row of `2023-03-01` ([`COUNTER_END_ANCHOR_BLOCK`]) to be
/// strictly smaller. This block exists **to be excluded**: the counter's
/// half-open `[T-24h, T)` window must drop it from both the burn anchor and
/// the tips sum (decision record `20260811-1435/finding-01`).
const COUNTER_WINDOW_EDGE_BLOCK: i64 = 207;

/// Gas price of the counter-window fixture's priced transactions. Strictly
/// above [`BASE_FEE_PER_GAS`] so every tip is positive and hand-computable.
const COUNTER_TX_GAS_PRICE: i64 = 5_000_000_000;
/// Gas used by the counter-window fixture's priced transactions.
const COUNTER_TX_GAS_USED: i64 = 21_000;

/// The counter-window fixture: `(block number, block timestamp, f099
/// balance in FIL, carries a priced transaction)`. Reproduces the six
/// measured properties of decision record
/// `test-fixture-for-address-coin-balances.md` §7:
///
/// 1. balance rows on blocks inside the counter's default window;
/// 2. anchors ([`COUNTER_START_ANCHOR_BLOCK`], [`COUNTER_END_ANCHOR_BLOCK`])
///    strictly inside both window edges;
/// 3. both densities — [`COUNTER_DENSE_BLOCK_1`]/`_2` sit five minutes
///    apart, [`COUNTER_END_ANCHOR_BLOCK`] is hours from its neighbours;
/// 4. [`COUNTER_DOMINANT_BLOCK`] carries most of the window's delta, next
///    to many near-zero deltas;
/// 5. `value IS NULL` rows ([`COUNTER_SPARSE_NULL_BLOCK_1`]/`_2`)
///    interleaved with real ones (pins the `value IS NOT NULL` guard);
/// 6. 26-digit wei balances (comes free: every value here is `fil * ETHER`,
///    same as [`burn_actor_balances_fil`]).
fn counter_window_fixture() -> Vec<(i64, &'static str, Option<i128>, bool)> {
    vec![
        (
            COUNTER_START_ANCHOR_BLOCK,
            "2023-02-28T11:50:00",
            Some(30_036_000),
            false,
        ),
        (
            COUNTER_DENSE_BLOCK_1,
            "2023-02-28T12:05:00",
            Some(30_036_050),
            false,
        ),
        (
            COUNTER_DENSE_BLOCK_2,
            "2023-02-28T12:10:00",
            Some(30_036_100),
            true,
        ),
        (
            COUNTER_SPARSE_NULL_BLOCK_1,
            "2023-02-28T15:00:00",
            None,
            false,
        ),
        (
            COUNTER_DOMINANT_BLOCK,
            "2023-02-28T20:00:00",
            Some(30_047_000),
            false,
        ),
        (
            COUNTER_SPARSE_NULL_BLOCK_2,
            "2023-03-01T08:00:00",
            None,
            false,
        ),
        (
            COUNTER_END_ANCHOR_BLOCK,
            "2023-03-01T11:50:00",
            Some(30_049_000),
            true,
        ),
        (
            COUNTER_WINDOW_EDGE_BLOCK,
            "2023-03-01T12:00:00",
            Some(30_050_000),
            true,
        ),
    ]
}

fn mock_address_coin_balance(
    addr: Vec<u8>,
    block_number: i64,
    value: Option<i128>,
) -> address_coin_balances::ActiveModel {
    address_coin_balances::ActiveModel {
        address_hash: Set(addr),
        block_number: Set(block_number),
        value: Set(value.map(Decimal::from)),
        value_fetched_at: Set(None),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
    }
}

/// Builds the blocks, transactions and per-block f099 balance rows of
/// [`counter_window_fixture`], truncated at `max_date` like every other
/// dated data set of this layer.
fn counter_window_blocks_transactions_and_balances(
    max_date: NaiveDate,
) -> (
    Vec<blocks::ActiveModel>,
    Vec<transactions::ActiveModel>,
    Vec<address_coin_balances::ActiveModel>,
) {
    let addresses = mock_addresses();
    let burn_actor_hash = hex::decode(BURN_ACTOR_HASH_HEX).unwrap();
    assert_eq!(burn_actor_hash.len(), 20, "f099 hash must be 20 bytes");

    let mut new_blocks = Vec::new();
    let mut new_transactions = Vec::new();
    let mut new_balances = Vec::new();
    for (number, ts, value, has_tx) in counter_window_fixture() {
        let ts = NaiveDateTime::from_str(ts).unwrap();
        // Same rule as `mock_burn_actor_balances`: the layer must not put
        // blocks or per-block rows after the date the indexer is pretended
        // to have reached, or the daily and per-block views of the f099
        // balance stop agreeing.
        if ts.date() > max_date {
            continue;
        }
        let block = mock_block(number, ts, true);
        if has_tx {
            new_transactions.push(mock_transaction(
                &block,
                COUNTER_TX_GAS_USED,
                COUNTER_TX_GAS_PRICE,
                &addresses,
                0,
                TxType::Transfer,
            ));
        }
        new_balances.push(mock_address_coin_balance(
            burn_actor_hash.clone(),
            number,
            value.map(|fil| fil * ETHER as i128),
        ));
        new_blocks.push(block);
    }
    (new_blocks, new_transactions, new_balances)
}

fn mixed_day_blocks_and_transactions() -> (Vec<blocks::ActiveModel>, Vec<transactions::ActiveModel>)
{
    let addresses = mock_addresses();
    let priced_block = mock_block(
        MIXED_DAY_PRICED_BLOCK,
        NaiveDateTime::from_str(&format!("{MIXED_DAY}T10:00:00")).unwrap(),
        true,
    );
    let hazard_block = mock_block(
        MIXED_DAY_HAZARD_BLOCK,
        NaiveDateTime::from_str(&format!("{MIXED_DAY}T12:00:00")).unwrap(),
        true,
    );
    let transactions = vec![
        // the only tip term of the day that survives
        mock_transaction(
            &priced_block,
            MIXED_DAY_COUNTED_GAS_USED,
            MIXED_DAY_GAS_PRICE,
            &addresses,
            0,
            TxType::Transfer,
        ),
        // keeps the hazard block's `base_fee_per_gas` NULL (hazard rule)
        mock_transaction(&hazard_block, 21_000, 0, &addresses, 0, TxType::Transfer),
        // lost: the NULL base fee makes this tip term NULL, `SUM` skips it
        mock_transaction(
            &hazard_block,
            MIXED_DAY_LOST_GAS_USED,
            MIXED_DAY_GAS_PRICE,
            &addresses,
            1,
            TxType::Transfer,
        ),
    ];
    (vec![priced_block, hazard_block], transactions)
}

/// Burn-actor (f099) balances per day, in whole FIL.
///
/// Monotonically increasing and deliberately sparse:
/// - `2022-11-11` has no row while FEVM transactions exist that day —
///   a "tips-only" day exercising the `FillPrevious` carry-forward;
/// - `2022-12-15` has neither an f099 row nor any block — the genuine
///   no-data day asserted by absence at chart level and by filled values
///   at the API level;
/// - `2023-02-28` is [`COUNTER_DOMINANT_BLOCK`]'s day: the value is that
///   block's per-block reading, the last one of the day — required to
///   agree with [`counter_window_fixture`] (decision record
///   `20260811-1040/finding-03`);
/// - `2023-03-01` has a row while its *original* only block (12) keeps
///   `base_fee_per_gas = NULL` (see the hazard rule below) — a natural
///   "burn-only" day; the value now equals [`COUNTER_WINDOW_EDGE_BLOCK`]'s
///   per-block reading, the last one of the day.
fn burn_actor_balances_fil() -> Vec<(NaiveDate, i128)> {
    [
        ("2022-11-09", 30_000_000),
        ("2022-11-10", 30_001_000),
        ("2022-11-12", 30_003_500),
        ("2022-12-01", 30_010_000),
        ("2023-01-01", 30_020_000),
        ("2023-02-01", 30_035_000),
        ("2023-02-28", 30_047_000),
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
            mock_address_coin_balance_daily(burn_actor_hash.clone(), day, Some(fil * ETHER as i128))
        })
        .collect()
}

/// Fills Filecoin-specific data on top of the shared fixture
/// ([`super::mock_blockscout::fill_mock_blockscout_data`], which must have
/// been applied already with the same `max_date`):
///
/// - inserts f099 `address_coin_balances_daily` rows
///   (see [`burn_actor_balances_fil`]);
/// - inserts the [`MIXED_DAY`] blocks and transactions
///   (see [`mixed_day_blocks_and_transactions`]);
/// - inserts the 24-hour counter's window blocks, transactions and per-block
///   `address_coin_balances` rows (see [`counter_window_fixture`]);
/// - sets [`BASE_FEE_PER_GAS`] on every consensus block that carries
///   transactions, **except** blocks carrying at least one
///   `gas_price = 0` transaction ("hazard" blocks, which keep NULL).
///
/// The hazard rule mirrors a fixture-only artifact: a positive base fee
/// under a zero-priced fixture transaction would produce a negative tip,
/// which the charts' per-transaction tip floor would clamp to 0 anyway —
/// keeping such blocks NULL instead exercises the NULL-handling paths.
/// Hazard blocks that are the only block of their day (block 0 on
/// `2022-11-09`, block 12 on `2023-03-01`) leave all tip terms of their
/// day NULL, so the day is dropped by the `value IS NOT NULL` guard of
/// `fevmFeeTips`; the hazard block of [`MIXED_DAY`] shares its day with a
/// priced block, so that day survives with an understated sum.
pub async fn fill_mock_blockscout_filecoin_data(
    blockscout: &DatabaseConnection,
    max_date: NaiveDate,
) {
    address_coin_balances_daily::Entity::insert_many(mock_burn_actor_balances(max_date))
        .exec(blockscout)
        .await
        .unwrap();

    if NaiveDate::from_str(MIXED_DAY).unwrap() <= max_date {
        let (mixed_blocks, mixed_transactions) = mixed_day_blocks_and_transactions();
        blocks::Entity::insert_many(mixed_blocks)
            .exec(blockscout)
            .await
            .unwrap();
        transactions::Entity::insert_many(mixed_transactions)
            .exec(blockscout)
            .await
            .unwrap();
    }

    // `2023-02-28` must stay equal to the earliest date in
    // `counter_window_fixture()`: guard-true then implies at least one row
    // of each kind survives the `max_date` filter inside the builder, so no
    // `insert_many` below is ever called with an empty vector.
    if NaiveDate::from_str("2023-02-28").unwrap() <= max_date {
        let (counter_blocks, counter_transactions, counter_balances) =
            counter_window_blocks_transactions_and_balances(max_date);
        blocks::Entity::insert_many(counter_blocks)
            .exec(blockscout)
            .await
            .unwrap();
        transactions::Entity::insert_many(counter_transactions)
            .exec(blockscout)
            .await
            .unwrap();
        address_coin_balances::Entity::insert_many(counter_balances)
            .exec(blockscout)
            .await
            .unwrap();
    }

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
    use std::collections::{HashMap, HashSet};

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tests::{init_db::init_db_blockscout, mock_blockscout::fill_mock_blockscout_data};

    /// Timestamps at which a counter test resolves an anchor bound: the
    /// start and end of every 24-hour window a test asks for. The bound is
    /// `max(blocks.number)` under a `timestamp < edge` filter (handoff §8),
    /// so at each of these instants the highest-numbered consensus block
    /// must also be the newest one — otherwise the anchors describe a chain
    /// that cannot exist. See `block_numbers_follow_time_at_counter_edges`.
    ///
    /// Add both edges of any new **literal** `update_time` used by a
    /// counter test. **Wall-clock** windows (the stats-server integration
    /// suite updates at `Utc::now()`, `update_time_override: None`) cannot
    /// be listed literally — they are covered by the sentinel entry below
    /// instead.
    const BOUND_EDGES: [&str; 7] = [
        // default window: update_time = max_time = 2023-03-01T12:00:00Z
        "2023-02-28T12:00:00",
        "2023-03-01T12:00:00",
        // "no f099 rows in the window" scenario (Phase 2)
        "2022-11-09T12:00:00",
        "2022-11-10T12:00:00",
        // "degenerate anchors" scenario (Phase 2): update_time =
        // 2023-03-03T00:00:00, both edges land after the fixture's last
        // per-block row (COUNTER_WINDOW_EDGE_BLOCK, 2023-03-01T12:00:00),
        // so both anchors resolve to that same row
        "2023-03-02T00:00:00",
        "2023-03-03T00:00:00",
        // Sentinel for the wall-clock class: stands for *every* instant
        // after the fixture's last consensus block. The stats-server
        // integration tests resolve their bounds at the real current time,
        // which cannot be listed literally — but the fixture is static, so
        // all post-fixture edges induce the same qualifying block set and
        // this one entry covers them all. The date is symbolic. The proof
        // transfers to the integration suite because its DB is built by the
        // same fill functions with the same `max_date`
        // (`mock_blockscout_simple/mod.rs:48-59`).
        "2100-01-01T00:00:00",
    ];

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

        // the per-block series and the daily series are two views of one
        // monotonic balance (decision record
        // `test-fixture-for-address-coin-balances.md` §3.5): the daily
        // value of a day must equal the last per-block value of that day.
        let daily: HashMap<NaiveDate, i128> = balances.into_iter().collect();

        // The day-by-day agreement check below relies on
        // `counter_window_fixture` being ordered by increasing timestamp
        // (and block number), so the last insert per day wins, and on its
        // non-NULL values being monotonically increasing (the same balance
        // series as `burn_actor_balances_fil`, just read per-block). Prove
        // both explicitly rather than only asserting them in a comment.
        let per_block = counter_window_fixture();
        assert!(
            per_block.windows(2).all(|w| w[0].0 < w[1].0
                && NaiveDateTime::from_str(w[0].1).unwrap()
                    < NaiveDateTime::from_str(w[1].1).unwrap()),
            "counter_window_fixture must be sorted by increasing block number and timestamp"
        );
        assert!(
            per_block
                .iter()
                .filter_map(|(_, _, value, _)| *value)
                .collect::<Vec<_>>()
                .windows(2)
                .all(|w| w[0] < w[1]),
            "counter_window_fixture's non-NULL values must be monotonically increasing"
        );

        let mut last_per_block_value_by_day: HashMap<NaiveDate, i128> = HashMap::new();
        for (_, ts, value, _) in per_block {
            let Some(value) = value else { continue };
            let ts = NaiveDateTime::from_str(ts).unwrap();
            // Ordering just proven above, so the last insert per day wins.
            last_per_block_value_by_day.insert(ts.date(), value);
        }
        assert!(!last_per_block_value_by_day.is_empty());
        for (day, per_block_value) in last_per_block_value_by_day {
            assert_eq!(
                daily.get(&day).copied(),
                Some(per_block_value),
                "daily value of {day} must equal the last per-block value of that day"
            );
        }
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

    /// Proves, independently of the counter's own SQL, that at every edge in
    /// [`BOUND_EDGES`] the anchor CTEs' `max(blocks.number)` bound agrees
    /// with the block that is actually newest by timestamp. A violation
    /// means the fixture's block numbers and timestamps disagree at that
    /// edge, so the anchors the counter would pick describe a chain that
    /// cannot exist (decision record `20260811-1040/finding-05`,
    /// `20260811-1040/finding-06`).
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn block_numbers_follow_time_at_counter_edges() {
        let blockscout = init_db_blockscout("mock_blockscout_filecoin_bound_order").await;
        let max_date = NaiveDate::from_str("2023-03-01").unwrap();
        fill_mock_blockscout_data(&blockscout, max_date).await;
        fill_mock_blockscout_filecoin_data(&blockscout, max_date).await;

        for edge in BOUND_EDGES {
            let edge_ts = NaiveDateTime::from_str(edge).unwrap();
            // left: the bound the counter's SQL computes; right: the same
            // bound derived from the semantic rule ("last consensus block
            // before this instant"), independently of `number`.
            let row = blockscout
                .query_one(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    r#"
                        SELECT
                            (
                                SELECT max(number) FROM blocks
                                WHERE consensus = true
                                  AND timestamp != to_timestamp(0)
                                  AND timestamp < $1
                            ) AS by_number,
                            (
                                SELECT number FROM blocks
                                WHERE consensus = true
                                  AND timestamp != to_timestamp(0)
                                  AND timestamp < $1
                                ORDER BY timestamp DESC, number DESC LIMIT 1
                            ) AS by_time
                    "#,
                    vec![edge_ts.into()],
                ))
                .await
                .unwrap()
                .unwrap();
            let by_number: Option<i64> = row.try_get_by("by_number").unwrap();
            let by_time: Option<i64> = row.try_get_by("by_time").unwrap();
            // `None == None` is agreement: no qualifying block, so the
            // counter gets a NULL bound and falls back — a tested scenario.
            assert_eq!(
                by_number, by_time,
                "at {edge} the highest-numbered consensus block is not the \
                 newest one: the anchor bounds would describe an impossible \
                 chain",
            );
        }
    }
}
