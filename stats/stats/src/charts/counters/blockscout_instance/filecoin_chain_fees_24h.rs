// SPDX-License-Identifier: LicenseRef-Blockscout

//! Internal 24-hour chain-fees counter for Filecoin: `max(burn, 0) + tips`
//! over the last 24 hours, queried directly from the indexer database once
//! per hour.
//!
//! ## Two different window boundaries, on purpose
//!
//! The two summands do **not** share the same interval boundary, and this is
//! deliberate, not an oversight (decision record
//! `pre-plan-handoff/burn-tips-window-skew.md`):
//!
//! - the **burn** part is the balance growth of the f099 burn actor between
//!   two *anchor blocks* — the last per-block `address_coin_balances` row at
//!   or before each edge of the window;
//! - the **tips** part is the sum of FEVM miner tips over the *exact*
//!   timestamp interval, same as `fevmFeeTips`.
//!
//! An anchor is therefore always a little older than its edge. On Filecoin
//! mainnet (handoff §4) the resulting skew is minutes-scale, two-sided (each
//! anchor lags independently, so the errors tend to cancel), and
//! self-correcting: the counter recomputes over a fresh window every hour,
//! so minutes that fall outside one window's anchors fall inside the next
//! one's. [`Properties::map_function`]'s warning on a large skew
//! ([`REASON_ANCHOR_SPAN_SKEW`]) is this decision's stress signal, not a
//! sign that the design is wrong.
//!
//! ## Internal-only, served under a shared public id
//!
//! Never exposed under its own id (its `charts.json` entry stays
//! `"enabled": false`, mirroring `fevm_fee_tips.rs`'s status): it is served
//! under the existing public id `txns_fee_24h` through the `implementation`
//! remap, on instances with `STATS__ENABLE_ALL_FILECOIN=true` (Phase 3/4 of
//! this plan; decision record
//! `pre-plan-handoff/public-id-remap-vs-own-entry.md`).
//!
//! ## The half-open window `[T-24h, T)`
//!
//! [`PullOne24hCached`] derives the window from the update time `T` and
//! hands this statement a range that is exclusive at `T` only because of a
//! numeric accident, not because the framework asks for a closed one: it
//! builds `[T-24h, T+1ns)` (`utils::interval_24h`, then
//! `range::inclusive_range_to_exclusive`), and Postgres timestamp columns
//! (microsecond precision) plus the driver's truncating encoder collapse
//! `T+1ns` back down to `T` on the wire. So the effective window is
//! `[T-24h, T)`. A block dated exactly `T` must therefore be excluded from
//! both the burn anchor and the tips sum — the fixture's
//! `COUNTER_WINDOW_EDGE_BLOCK` (Phase 1) exists to prove exactly that (see
//! the default-window DB test below and decision record
//! `comments/decisions/20260811-1435/finding-01-half-open-window-precision.md`).

use std::{collections::HashSet, ops::Range};

use tracing::warn;

use crate::{chart_prelude::*, lines::BURN_ACTOR_HASH_HEX, utils::ETHER};

pub struct FilecoinChainFees24hStatement;
impl_db_choice!(FilecoinChainFees24hStatement, UsePrimaryDB);

impl StatementFromRange for FilecoinChainFees24hStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &IndexerMigrations,
        _enabled_update_charts_recursive: &HashSet<ChartKey>,
    ) -> Statement {
        // `PullOne24hCached` always supplies a concrete 24-hour window
        // derived from the update time; this statement is not otherwise
        // reachable, so there is no meaningful fallback for `None`.
        let range = range.expect(
            "FilecoinChainFees24hStatement is only ever driven by PullOne24hCached, \
             which always supplies a concrete range",
        );
        let args = vec![
            ETHER.into(),
            BURN_ACTOR_HASH_HEX.into(),
            range.end.into(),
            range.start.into(),
        ];
        // see `statement_denormalized_is_correct` / `statement_non_denormalized_is_correct`
        // for the resulting SQL text
        //
        // `from_block_time`/`to_block_time` are scalar subqueries on purpose:
        // `blocks` has a UNIQUE partial index on `(number) WHERE consensus`
        // (`one_consensus_block_at_height` in the blockscout-db schema dump),
        // and `consensus` is NOT NULL, so `consensus = true AND number =
        // <one value>` matches at most one row by construction. No `LIMIT 1`
        // — a duplicate here would mean the core Blockscout unique index is
        // gone, and that should fail loudly rather than be silently papered
        // over.
        let sql = if completed_migrations.denormalization {
            r#"
                WITH bound_to AS (
                    SELECT max(number) AS number FROM blocks
                    WHERE consensus = true AND timestamp != to_timestamp(0) AND timestamp < $3
                ),
                bound_from AS (
                    SELECT max(number) AS number FROM blocks
                    WHERE consensus = true AND timestamp != to_timestamp(0) AND timestamp < $4
                ),
                anchor_to AS (
                    SELECT block_number, value FROM address_coin_balances
                    WHERE address_hash = decode($2, 'hex')
                        AND value IS NOT NULL
                        AND block_number <= (SELECT number FROM bound_to)
                    ORDER BY block_number DESC LIMIT 1
                ),
                anchor_from AS (
                    SELECT block_number, value FROM address_coin_balances
                    WHERE address_hash = decode($2, 'hex')
                        AND value IS NOT NULL
                        AND block_number <= (SELECT number FROM bound_from)
                    ORDER BY block_number DESC LIMIT 1
                ),
                tips AS (
                    SELECT
                        SUM(
                            CASE
                                WHEN t_filtered.gas_price < b.base_fee_per_gas THEN 0
                                ELSE (t_filtered.gas_price - b.base_fee_per_gas) * t_filtered.gas_used
                            END
                        ) AS wei,
                        COUNT(*) AS txns
                    FROM (
                        SELECT * FROM transactions t
                        WHERE
                            t.block_consensus = true AND
                            t.block_timestamp != to_timestamp(0) AND
                            t.block_timestamp < $3 AND
                            t.block_timestamp >= $4
                    ) AS t_filtered
                    JOIN blocks b ON t_filtered.block_hash = b.hash
                    WHERE
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true AND
                        b.timestamp < $3 AND
                        b.timestamp >= $4
                )
                SELECT
                    ((SELECT value FROM anchor_to) - (SELECT value FROM anchor_from))::float / $1 AS burn,
                    (SELECT wei FROM tips)::float / $1 AS tips,
                    (SELECT txns FROM tips) AS tips_txns,
                    (SELECT block_number FROM anchor_from) AS from_block,
                    (SELECT block_number FROM anchor_to) AS to_block,
                    (SELECT timestamp FROM blocks
                        WHERE consensus = true AND number = (SELECT block_number FROM anchor_from)) AS from_block_time,
                    (SELECT timestamp FROM blocks
                        WHERE consensus = true AND number = (SELECT block_number FROM anchor_to)) AS to_block_time
            "#
        } else {
            r#"
                WITH bound_to AS (
                    SELECT max(number) AS number FROM blocks
                    WHERE consensus = true AND timestamp != to_timestamp(0) AND timestamp < $3
                ),
                bound_from AS (
                    SELECT max(number) AS number FROM blocks
                    WHERE consensus = true AND timestamp != to_timestamp(0) AND timestamp < $4
                ),
                anchor_to AS (
                    SELECT block_number, value FROM address_coin_balances
                    WHERE address_hash = decode($2, 'hex')
                        AND value IS NOT NULL
                        AND block_number <= (SELECT number FROM bound_to)
                    ORDER BY block_number DESC LIMIT 1
                ),
                anchor_from AS (
                    SELECT block_number, value FROM address_coin_balances
                    WHERE address_hash = decode($2, 'hex')
                        AND value IS NOT NULL
                        AND block_number <= (SELECT number FROM bound_from)
                    ORDER BY block_number DESC LIMIT 1
                ),
                tips AS (
                    SELECT
                        SUM(
                            CASE
                                WHEN t.gas_price < b.base_fee_per_gas THEN 0
                                ELSE (t.gas_price - b.base_fee_per_gas) * t.gas_used
                            END
                        ) AS wei,
                        COUNT(*) AS txns
                    FROM transactions t
                    JOIN blocks b ON t.block_hash = b.hash
                    WHERE
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true AND
                        b.timestamp < $3 AND
                        b.timestamp >= $4
                )
                SELECT
                    ((SELECT value FROM anchor_to) - (SELECT value FROM anchor_from))::float / $1 AS burn,
                    (SELECT wei FROM tips)::float / $1 AS tips,
                    (SELECT txns FROM tips) AS tips_txns,
                    (SELECT block_number FROM anchor_from) AS from_block,
                    (SELECT block_number FROM anchor_to) AS to_block,
                    (SELECT timestamp FROM blocks
                        WHERE consensus = true AND number = (SELECT block_number FROM anchor_from)) AS from_block_time,
                    (SELECT timestamp FROM blocks
                        WHERE consensus = true AND number = (SELECT block_number FROM anchor_to)) AS to_block_time
            "#
        };
        Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
    }
}

/// One row pulled by [`FilecoinChainFees24hStatement`]: the two burn
/// anchors and the tips aggregate over the requested 24-hour window.
///
/// Every field but [`tips_txns`](Self::tips_txns) is nullable: an empty
/// window, a missing anchor (the `value IS NOT NULL` guard drops rows whose
/// balance fetch never completed, or there may be no qualifying block at
/// all), or a window with no priced transaction all surface as `NULL` from
/// the query rather than as an error — [`Properties`]'s map function turns
/// each of those into a documented fallback instead of failing the update.
#[derive(Debug, Clone, FromQueryResult)]
pub struct FilecoinChainFees24hValue {
    /// FIL burn between the two anchors (`anchor_to.value - anchor_from.value`).
    /// `NULL` when either anchor is missing.
    pub burn: Option<f64>,
    /// FEVM miner tips over the exact window, in FIL. `NULL` when every
    /// block in the window that carries a transaction still lacks
    /// `base_fee_per_gas`; see [`tips_txns`](Self::tips_txns) to tell that
    /// apart from "no transactions in the window at all".
    pub tips: Option<f64>,
    /// Number of transactions found in the tips window. Never `NULL`:
    /// `COUNT(*)` over an aggregate with zero matching rows is `0`, not
    /// `NULL`. Zero means no transactions in the window; a positive value
    /// together with `tips = NULL` means transactions exist but the base
    /// fee is missing on every block that carries one.
    pub tips_txns: i64,
    /// Block number of the start anchor (`bound_from`'s last non-null f099
    /// row). `NULL` when no such row exists.
    pub from_block: Option<i64>,
    /// Block number of the end anchor (`bound_to`'s last non-null f099
    /// row). `NULL` when no such row exists.
    pub to_block: Option<i64>,
    /// Timestamp of the start anchor block. `NULL` when `from_block` is
    /// `NULL` — or, in principle, when the anchor's height currently has no
    /// consensus block (the anchor CTEs do not filter on the block's
    /// `consensus` flag); in that shape the burn is still computed and used
    /// while the update is routed to the degenerate-anchors warning.
    pub from_block_time: Option<NaiveDateTime>,
    /// Timestamp of the end anchor block. `NULL` under the same conditions
    /// as [`from_block_time`](Self::from_block_time), for the end anchor.
    pub to_block_time: Option<NaiveDateTime>,
}

pub type FilecoinChainFees24hRemote = RemoteDatabaseSource<
    PullOne24hCached<FilecoinChainFees24hStatement, FilecoinChainFees24hValue>,
>;

/// Machine-readable selector for the anchor-span-skew warning (see
/// [`CombineBurnAndTips`]). Log-based alerting keys on this field, so the
/// warning's prose message stays free to change.
pub const REASON_ANCHOR_SPAN_SKEW: &str = "anchor_span_skew";
/// Machine-readable selector for the degenerate/missing-anchors warning
/// (see [`CombineBurnAndTips`]).
pub const REASON_DEGENERATE_ANCHORS: &str = "degenerate_anchors";

/// Warning threshold, in minutes, for how far the span between the two burn
/// anchors' timestamps may deviate from the nominal 24 hours before
/// [`CombineBurnAndTips`] logs [`REASON_ANCHOR_SPAN_SKEW`].
///
/// Calibrated from Filecoin **mainnet** anchor lags (handoff §4: 4 minutes
/// median, 16 minutes p90, 47 minutes maximum, sampled over 26.3 hours on
/// the f099 burn actor) — deliberately *not* from testnet, whose
/// one-row-per-block density (decision record
/// `pre-plan-handoff/test-fixture-for-address-coin-balances.md` §3.2) would
/// suggest a threshold roughly 40x tighter than mainnet actually needs.
pub const ANCHOR_SPAN_SKEW_WARNING_THRESHOLD_MINUTES: i64 = 60;

pub struct CombineBurnAndTips;

/// Emits a warning about the burn anchors, if warranted, then returns
/// `(burn, tips)` each with a missing summand replaced by `0.0`.
///
/// The two conditions are checked in a fixed order: degenerate/missing
/// anchors first. A degenerate window (both anchors resolve to the same
/// block) has a span of exactly `0`, which would *also* satisfy the skew
/// predicate below — checking degeneracy first keeps that case from firing
/// both warnings, and at most one warning is ever emitted per update.
///
/// Degeneracy is classified by *timestamp*, not block number: two distinct
/// anchor blocks sharing one timestamp give the window no usable time span,
/// so they land in the degenerate branch by design — the computed burn is
/// still used in that shape, which is why the warning carries a `burn`
/// field instead of asserting a numeric outcome in prose.
fn warn_on_anchor_issues(value: &FilecoinChainFees24hValue) {
    match (value.from_block_time, value.to_block_time) {
        (Some(from_time), Some(to_time)) if from_time != to_time => {
            let span = to_time - from_time;
            let deviation_from_24h = (span - TimeDelta::hours(24)).abs();
            if deviation_from_24h > TimeDelta::minutes(ANCHOR_SPAN_SKEW_WARNING_THRESHOLD_MINUTES) {
                warn!(
                    reason = REASON_ANCHOR_SPAN_SKEW,
                    from_block = value.from_block,
                    to_block = value.to_block,
                    from_block_time = %from_time,
                    to_block_time = %to_time,
                    span_seconds = span.num_seconds(),
                    tips_txns = value.tips_txns,
                    "the two burn anchors span noticeably more or less than 24 hours"
                );
            }
        }
        _ => {
            warn!(
                reason = REASON_DEGENERATE_ANCHORS,
                from_block = value.from_block,
                to_block = value.to_block,
                from_block_time = ?value.from_block_time,
                to_block_time = ?value.to_block_time,
                burn = ?value.burn,
                tips_txns = value.tips_txns,
                "the burn anchors do not span the window: one or both anchor timestamps are unresolved, or both carry the same block timestamp"
            );
        }
    }
}

impl MapFunction<TimespanValue<NaiveDate, FilecoinChainFees24hValue>> for CombineBurnAndTips {
    type Output = TimespanValue<NaiveDate, f64>;

    fn function(
        inner_data: TimespanValue<NaiveDate, FilecoinChainFees24hValue>,
    ) -> Result<Self::Output, ChartError> {
        let TimespanValue { timespan, value } = inner_data;
        warn_on_anchor_issues(&value);
        // each summand contributes 0 independently of the other when
        // missing, matching `txnsFee24h`'s `UnwrapOr<_, Zero>` semantics;
        // the burn side is additionally floored at zero, since the f099
        // balance only ever grows on-chain (mirrors `ClampNonNegative` over
        // `filecoin_new_chain_fees`'s burn delta)
        let burn = value.burn.unwrap_or(0.0).max(0.0);
        let tips = value.tips.unwrap_or(0.0);
        Ok(TimespanValue {
            timespan,
            value: burn + tips,
        })
    }
}

pub type FilecoinChainFees24hExtracted = Map<FilecoinChainFees24hRemote, CombineBurnAndTips>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "filecoinChainFees24h".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
    // Keeps the public id `txns_fee_24h` from waiting on the indexer on
    // Filecoin alone: under the `implementation` remap (Phase 3/4) the
    // remapped public entry runs with *this* struct's requirement, not
    // `txns_fee_24h.rs`'s own — and every other 24-hour counter in the
    // service already states this same value (decision record
    // `pre-plan-handoff/indexing-status-requirement-least-restrictive-vs-default.md`).
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::LEAST_RESTRICTIVE
    }
}

pub type FilecoinChainFees24h =
    DirectPointLocalDbChartSource<MapToString<FilecoinChainFees24hExtracted>, Properties>;

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::{Arc, Mutex},
    };

    use pretty_assertions::assert_eq;
    use tracing::field::{Field, Visit};
    use tracing_subscriber::layer::{Context, SubscriberExt};

    use super::*;
    use crate::tests::{
        normalize_sql,
        point_construction::dt,
        simple_test::{
            simple_test_counter_filecoin, simple_test_counter_filecoin_with_migration_variants,
        },
    };

    #[test]
    fn statement_denormalized_is_correct() {
        let actual = FilecoinChainFees24hStatement::get_statement(
            Some(dt("2023-01-01T00:00:00").and_utc()..dt("2023-01-02T00:00:00").and_utc()),
            &IndexerMigrations::latest(),
            &HashSet::new(),
        );
        let expected = r#"
            WITH bound_to AS (
                SELECT max(number) AS number FROM blocks
                WHERE consensus = true AND timestamp != to_timestamp(0) AND timestamp < '2023-01-02 00:00:00.000000 +00:00'
            ),
            bound_from AS (
                SELECT max(number) AS number FROM blocks
                WHERE consensus = true AND timestamp != to_timestamp(0) AND timestamp < '2023-01-01 00:00:00.000000 +00:00'
            ),
            anchor_to AS (
                SELECT block_number, value FROM address_coin_balances
                WHERE address_hash = decode('ff00000000000000000000000000000000000063', 'hex')
                    AND value IS NOT NULL
                    AND block_number <= (SELECT number FROM bound_to)
                ORDER BY block_number DESC LIMIT 1
            ),
            anchor_from AS (
                SELECT block_number, value FROM address_coin_balances
                WHERE address_hash = decode('ff00000000000000000000000000000000000063', 'hex')
                    AND value IS NOT NULL
                    AND block_number <= (SELECT number FROM bound_from)
                ORDER BY block_number DESC LIMIT 1
            ),
            tips AS (
                SELECT
                    SUM(
                        CASE
                            WHEN t_filtered.gas_price < b.base_fee_per_gas THEN 0
                            ELSE (t_filtered.gas_price - b.base_fee_per_gas) * t_filtered.gas_used
                        END
                    ) AS wei,
                    COUNT(*) AS txns
                FROM (
                    SELECT * FROM transactions t
                    WHERE
                        t.block_consensus = true AND
                        t.block_timestamp != to_timestamp(0) AND
                        t.block_timestamp < '2023-01-02 00:00:00.000000 +00:00' AND
                        t.block_timestamp >= '2023-01-01 00:00:00.000000 +00:00'
                ) AS t_filtered
                JOIN blocks b ON t_filtered.block_hash = b.hash
                WHERE
                    b.timestamp != to_timestamp(0) AND
                    b.consensus = true AND
                    b.timestamp < '2023-01-02 00:00:00.000000 +00:00' AND
                    b.timestamp >= '2023-01-01 00:00:00.000000 +00:00'
            )
            SELECT
                ((SELECT value FROM anchor_to) - (SELECT value FROM anchor_from))::float / 1000000000000000000 AS burn,
                (SELECT wei FROM tips)::float / 1000000000000000000 AS tips,
                (SELECT txns FROM tips) AS tips_txns,
                (SELECT block_number FROM anchor_from) AS from_block,
                (SELECT block_number FROM anchor_to) AS to_block,
                (SELECT timestamp FROM blocks
                    WHERE consensus = true AND number = (SELECT block_number FROM anchor_from)) AS from_block_time,
                (SELECT timestamp FROM blocks
                    WHERE consensus = true AND number = (SELECT block_number FROM anchor_to)) AS to_block_time
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()));
    }

    #[test]
    fn statement_non_denormalized_is_correct() {
        let actual = FilecoinChainFees24hStatement::get_statement(
            Some(dt("2023-01-01T00:00:00").and_utc()..dt("2023-01-02T00:00:00").and_utc()),
            &IndexerMigrations::empty(),
            &HashSet::new(),
        );
        let expected = r#"
            WITH bound_to AS (
                SELECT max(number) AS number FROM blocks
                WHERE consensus = true AND timestamp != to_timestamp(0) AND timestamp < '2023-01-02 00:00:00.000000 +00:00'
            ),
            bound_from AS (
                SELECT max(number) AS number FROM blocks
                WHERE consensus = true AND timestamp != to_timestamp(0) AND timestamp < '2023-01-01 00:00:00.000000 +00:00'
            ),
            anchor_to AS (
                SELECT block_number, value FROM address_coin_balances
                WHERE address_hash = decode('ff00000000000000000000000000000000000063', 'hex')
                    AND value IS NOT NULL
                    AND block_number <= (SELECT number FROM bound_to)
                ORDER BY block_number DESC LIMIT 1
            ),
            anchor_from AS (
                SELECT block_number, value FROM address_coin_balances
                WHERE address_hash = decode('ff00000000000000000000000000000000000063', 'hex')
                    AND value IS NOT NULL
                    AND block_number <= (SELECT number FROM bound_from)
                ORDER BY block_number DESC LIMIT 1
            ),
            tips AS (
                SELECT
                    SUM(
                        CASE
                            WHEN t.gas_price < b.base_fee_per_gas THEN 0
                            ELSE (t.gas_price - b.base_fee_per_gas) * t.gas_used
                        END
                    ) AS wei,
                    COUNT(*) AS txns
                FROM transactions t
                JOIN blocks b ON t.block_hash = b.hash
                WHERE
                    b.timestamp != to_timestamp(0) AND
                    b.consensus = true AND
                    b.timestamp < '2023-01-02 00:00:00.000000 +00:00' AND
                    b.timestamp >= '2023-01-01 00:00:00.000000 +00:00'
            )
            SELECT
                ((SELECT value FROM anchor_to) - (SELECT value FROM anchor_from))::float / 1000000000000000000 AS burn,
                (SELECT wei FROM tips)::float / 1000000000000000000 AS tips,
                (SELECT txns FROM tips) AS tips_txns,
                (SELECT block_number FROM anchor_from) AS from_block,
                (SELECT block_number FROM anchor_to) AS to_block,
                (SELECT timestamp FROM blocks
                    WHERE consensus = true AND number = (SELECT block_number FROM anchor_from)) AS from_block_time,
                (SELECT timestamp FROM blocks
                    WHERE consensus = true AND number = (SELECT block_number FROM anchor_to)) AS to_block_time
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()));
    }

    /// Builds a [`FilecoinChainFees24hValue`] with `Option<f64>` summands —
    /// pinning both fields as `Option<f64>` at compile time, so a summand
    /// typed bare `f64` (which would crash the hourly update on the first
    /// `NULL`, cf. `fevm_fee_tips.rs:41-43`) cannot even be written here.
    /// `tips_txns`/anchors are irrelevant to the clamp/NULL pin and fixed at
    /// harmless values; `from_time`/`to_time` drive the separate
    /// warning-kind pin.
    fn pulled(
        burn: Option<f64>,
        tips: Option<f64>,
        from_time: Option<NaiveDateTime>,
        to_time: Option<NaiveDateTime>,
    ) -> FilecoinChainFees24hValue {
        FilecoinChainFees24hValue {
            burn,
            tips,
            tips_txns: 0,
            from_block: from_time.map(|_| 1),
            to_block: to_time.map(|_| 2),
            from_block_time: from_time,
            to_block_time: to_time,
        }
    }

    fn combine(value: FilecoinChainFees24hValue) -> f64 {
        CombineBurnAndTips::function(TimespanValue {
            timespan: dt("2023-01-01T00:00:00").date(),
            value,
        })
        .unwrap()
        .value
    }

    #[test]
    fn combine_burn_and_tips_clamps_and_treats_null_as_zero() {
        let some_from = Some(dt("2023-01-01T00:00:00"));
        let some_to = Some(dt("2023-01-02T00:00:00"));

        // case 1: negative burn is clamped to 0 before adding tips — not
        // clamped *after* the sum. `|burn| > tips` here so a wrong
        // no-clamp implementation (-7.0) and a wrong clamp-after-sum
        // implementation (max(burn+tips, 0) = 0.0) both disagree with the
        // correct answer (3.0) and with each other.
        assert_eq!(
            combine(pulled(Some(-10.0), Some(3.0), some_from, some_to)),
            3.0
        );

        // case 2: pass-through when burn is non-negative — an over-eager
        // clamp cannot pass by zeroing everything.
        assert_eq!(
            combine(pulled(Some(5.0), Some(3.0), some_from, some_to)),
            8.0
        );

        // case 3: one-sided NULL tips -> 0, burn passes through unclamped.
        // Must stay non-zero: an implementation that zeroes the whole sum
        // whenever *any* summand is NULL would still pass every other
        // planned check (the DB tests all expect 0 anyway in their NULL
        // scenarios), so this is the only place that catches it.
        assert_eq!(combine(pulled(Some(5.0), None, some_from, some_to)), 5.0);

        // case 4: one-sided NULL burn -> 0, tips passes through.
        assert_eq!(combine(pulled(None, Some(3.0), some_from, some_to)), 3.0);
    }

    /// Collects the `reason` field of every event emitted while the
    /// subscriber is installed. Implemented from scratch (rather than
    /// reusing `request_id_layer.rs`'s model) because that model's
    /// `regex`/`serde_json` layers are not available in `stats` — this
    /// only needs one field's string value, not a full log line.
    #[derive(Default, Clone)]
    struct ReasonCollector {
        reasons: Arc<Mutex<Vec<String>>>,
    }

    struct ReasonVisitor<'a>(&'a mut Option<String>);

    impl Visit for ReasonVisitor<'_> {
        fn record_str(&mut self, field: &Field, value: &str) {
            if field.name() == "reason" {
                *self.0 = Some(value.to_string());
            }
        }

        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            if field.name() == "reason" {
                *self.0 = Some(format!("{value:?}"));
            }
        }
    }

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for ReasonCollector {
        fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
            let mut reason = None;
            event.record(&mut ReasonVisitor(&mut reason));
            if let Some(reason) = reason {
                self.reasons.lock().unwrap().push(reason);
            }
        }
    }

    /// Runs [`CombineBurnAndTips::function`] under a subscriber that
    /// records every `reason` field emitted, and returns the list in
    /// emission order. Does **not** snapshot message text or field
    /// formatting: the `reason` strings are the same `const`s the
    /// production `warn!` calls use, so this only breaks if the production
    /// behavior actually changes.
    fn reasons_emitted(value: FilecoinChainFees24hValue) -> Vec<String> {
        let collector = ReasonCollector::default();
        let reasons = collector.reasons.clone();
        let subscriber = tracing_subscriber::registry().with(collector);
        tracing::subscriber::with_default(subscriber, || {
            combine(value);
        });
        // `.lock()` rather than `Arc::try_unwrap`: other callsite-interest
        // bookkeeping inside `tracing` may briefly hold its own clone of a
        // just-uninstalled subscriber when many other tests in this binary
        // exercise `tracing` concurrently, so the strong count here is not
        // reliably 1 the moment `with_default`'s scope ends. The lock is
        // all this needs.
        reasons.lock().unwrap().clone()
    }

    /// Table-driven, and deliberately a **separate** `#[test]` from
    /// [`combine_burn_and_tips_clamps_and_treats_null_as_zero`]: value
    /// assertions and warning assertions must be able to fail
    /// independently. Spans are derived from
    /// [`ANCHOR_SPAN_SKEW_WARNING_THRESHOLD_MINUTES`], never as literals,
    /// so re-calibrating the constant moves the cases with it (decision
    /// record `comments/decisions/20260811-1435/finding-03-warning-contracts.md`).
    #[test]
    fn warning_kind_matches_anchor_shape() {
        let base_from = dt("2023-01-01T00:00:00");
        let threshold = TimeDelta::minutes(ANCHOR_SPAN_SKEW_WARNING_THRESHOLD_MINUTES);
        let at = |delta: TimeDelta| base_from + TimeDelta::hours(24) + delta;

        let cases: Vec<(&str, FilecoinChainFees24hValue, Vec<&str>)> = vec![
            (
                "span exactly 24h -> no warning",
                pulled(
                    Some(1.0),
                    Some(1.0),
                    Some(base_from),
                    Some(at(TimeDelta::zero())),
                ),
                vec![],
            ),
            (
                "span 24h + threshold -> no warning (boundary is inclusive of the pass)",
                pulled(Some(1.0), Some(1.0), Some(base_from), Some(at(threshold))),
                vec![],
            ),
            (
                "span 24h - threshold -> no warning",
                pulled(Some(1.0), Some(1.0), Some(base_from), Some(at(-threshold))),
                vec![],
            ),
            (
                "span 24h + threshold + 1min -> anchor_span_skew",
                pulled(
                    Some(1.0),
                    Some(1.0),
                    Some(base_from),
                    Some(at(threshold + TimeDelta::minutes(1))),
                ),
                vec![REASON_ANCHOR_SPAN_SKEW],
            ),
            (
                "span 24h - threshold - 1min -> anchor_span_skew",
                pulled(
                    Some(1.0),
                    Some(1.0),
                    Some(base_from),
                    Some(at(-threshold - TimeDelta::minutes(1))),
                ),
                vec![REASON_ANCHOR_SPAN_SKEW],
            ),
            (
                "missing start anchor -> degenerate_anchors",
                pulled(None, Some(1.0), None, Some(base_from)),
                vec![REASON_DEGENERATE_ANCHORS],
            ),
            (
                "missing end anchor -> degenerate_anchors",
                pulled(Some(1.0), None, Some(base_from), None),
                vec![REASON_DEGENERATE_ANCHORS],
            ),
            (
                // a zero span deviates from 24h by 24h, which would also
                // satisfy the skew predicate; this case exists to prove
                // the skew warning is absent here, i.e. the degenerate
                // check really does run first and suppress it. Note the
                // shape: `pulled` fabricates blocks 1 and 2, so these are
                // *distinct* anchor blocks sharing one timestamp — the
                // by-design degenerate classification by timestamp.
                "distinct blocks with equal timestamps -> degenerate_anchors only, not anchor_span_skew",
                pulled(Some(0.0), Some(1.0), Some(base_from), Some(base_from)),
                vec![REASON_DEGENERATE_ANCHORS],
            ),
            (
                // the genuine same-block shape: both anchors resolve to one
                // block, so the block numbers are equal too (unlike the row
                // above)
                "same anchor block -> degenerate_anchors",
                FilecoinChainFees24hValue {
                    to_block: Some(1),
                    ..pulled(Some(0.0), Some(1.0), Some(base_from), Some(base_from))
                },
                vec![REASON_DEGENERATE_ANCHORS],
            ),
        ];

        for (name, value, expected_reasons) in cases {
            let actual: Vec<String> = reasons_emitted(value);
            let expected: Vec<String> = expected_reasons.into_iter().map(String::from).collect();
            assert_eq!(actual, expected, "case: {name}");
        }
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_filecoin_chain_fees_24h_default_window() {
        // default window `[2023-02-28T12:00, 2023-03-01T12:00)`:
        // - start anchor: block 200 (2023-02-28T11:50), value 30_036_000 FIL
        // - end anchor: block 206 (2023-03-01T11:50), value 30_049_000 FIL
        //   (`COUNTER_WINDOW_EDGE_BLOCK`/207, dated exactly at the window's
        //   upper edge, must be excluded from the anchor — proving the
        //   half-open `[T-24h, T)` contract)
        // - burn = 30_049_000 - 30_036_000 = 13_000 FIL
        // - tips (wei sum): two priced transactions inside the window
        //   (blocks 202 and 206; block 207's transaction is excluded by the
        //   same half-open edge), each `(5_000_000_000 - 100_000_000) *
        //   21_000` wei = 102_900_000_000_000 wei, total 205_800_000_000_000
        //   wei = 0.0002058 FIL
        // - tips_txns = 4, not 2: the shared blockscout fixture's last block
        //   (`2023-03-01T10:00:00`, also in-window) carries two more
        //   transactions of its own (an attributes-deposit transaction with
        //   `gas_price = 0`, and a "dropped/replaced" failed transaction) —
        //   the zero-priced one makes that block a "hazard" block whose
        //   `base_fee_per_gas` stays NULL, so both transactions' tip terms
        //   are NULL and `SUM` skips them, but `COUNT(*)` still counts the
        //   joined rows, adding 2 to `tips_txns` without changing the wei
        //   sum
        // - total = 13_000.0002058
        //
        // Run through the migration-variants runner so the tips filter's
        // strict `<` at the window's upper edge is proven in both
        // hand-written schema forms (model: `update_fevm_fee_tips`;
        // decision record
        // `comments/decisions/20260811-1435/finding-01-half-open-window-precision.md`).
        simple_test_counter_filecoin_with_migration_variants::<FilecoinChainFees24h>(
            "update_filecoin_chain_fees_24h_default_window",
            "13000.0002058",
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_filecoin_chain_fees_24h_no_rows_in_window() {
        // window `[2022-11-09T12:00, 2022-11-10T12:00)`: no f099
        // `address_coin_balances` row exists anywhere near this window (the
        // per-block fixture starts at block 200, dated 2023-02-28), so both
        // anchors are NULL and burn falls back to 0 via the NULL->0 rule —
        // not a confident wrong number computed from an out-of-window
        // anchor. Both edges of this window are listed in `BOUND_EDGES`.
        //
        // The window is not otherwise empty: the shared fixture's
        // contract-creation transactions land two of its blocks (0 and 1,
        // 2022-11-09T23:59:59 and 2022-11-10T00:00:00) inside it, and the
        // Filecoin layer's closing `UPDATE` prices every consensus block
        // that carries a transaction, shared-fixture blocks included — so
        // the reported total is tips alone, not a bare `0`.
        simple_test_counter_filecoin::<FilecoinChainFees24h>(
            "update_filecoin_chain_fees_24h_no_rows_in_window",
            "0.000042985185138",
            Some(dt("2022-11-10T12:00:00")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_filecoin_chain_fees_24h_degenerate_anchors() {
        // window `[2023-03-02T00:00, 2023-03-03T00:00)`: both edges land
        // after the fixture's last per-block f099 row
        // (`COUNTER_WINDOW_EDGE_BLOCK`, 2023-03-01T12:00), so both anchors
        // resolve to that same row (value 30_050_000 FIL) — burn falls back
        // to `max(0, 0) = 0` via the equal-anchors path, not the
        // missing-anchor path. No transactions fall in this window either,
        // so tips is also 0. Both edges are listed in `BOUND_EDGES`.
        simple_test_counter_filecoin::<FilecoinChainFees24h>(
            "update_filecoin_chain_fees_24h_degenerate_anchors",
            "0",
            Some(dt("2023-03-03T00:00:00")),
        )
        .await;
    }

    /// Pins the *chosen anchors*, not only the final value: a sum-only test
    /// can pass on two compensating errors (decision record
    /// `pre-plan-handoff/test-fixture-for-address-coin-balances.md` §8).
    /// Queries [`PullOne24hCached`] directly, independently of
    /// [`CombineBurnAndTips`], against a database built the same way
    /// [`simple_test_counter_filecoin`] builds one.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn default_window_anchors_are_pinned() {
        use crate::{
            data_source::UpdateParameters,
            tests::{
                init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data,
                mock_blockscout_filecoin::fill_mock_blockscout_filecoin_data,
            },
        };
        use std::str::FromStr;

        let (db, indexer) = init_db_all("filecoin_chain_fees_24h_anchors_are_pinned").await;
        let max_date = NaiveDate::from_str("2023-03-01").unwrap();
        fill_mock_blockscout_data(&indexer, max_date).await;
        fill_mock_blockscout_filecoin_data(&indexer, max_date).await;

        let update_time = DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
        let parameters =
            UpdateParameters::default_test_query_parameters(&db, &indexer, Some(update_time));
        let cx = UpdateContext::from_params_now_or_override(parameters);

        let pulled = PullOne24hCached::<FilecoinChainFees24hStatement, FilecoinChainFees24hValue>::query_data(
            &cx,
            UniversalRange::full(),
        )
        .await
        .unwrap();
        // see `update_filecoin_chain_fees_24h_default_window` for the
        // reasoning behind these numbers
        assert_eq!(pulled.value.from_block, Some(200));
        assert_eq!(pulled.value.to_block, Some(206));
        assert_eq!(pulled.value.tips_txns, 4);
    }
}
