# ADR-009: Stats Chains Global-Plus-Per-Bridge Snapshots

**Date:** 2026-08-24

**Authors:** @EvgenKor

## Context

`GET /api/v1/stats/chains` / `GetChainsStats` reported `unique_transfer_users_count`
per chain from `stats_chains`, a snapshot keyed only by `chain_id` and rebuilt
periodically (`InterchainDatabase::recompute_stats_chains`) from distinct
`(chain_id, address)` pairs across **every** bridge's canonical rows. The
endpoint had no way to scope those counts to a caller-selected set of bridges.

The frontend needed exactly that: per-bridge unique-user counts per chain, for
dashboards that break down activity by bridge rather than by chain alone.
Doing that scoping client-side would require exposing raw per-message/transfer
rows or a second aggregate endpoint whose consistency with `/stats/chains`
would be the caller's problem, not the indexer's.

Two properties made a naive fix unsafe:

- **Unique-user counts are not additive.** The same address can appear in more
  than one bridge's canonical rows on the same chain (directly, or through a
  bridge configured with `process_unknown_chains = true`, which lets it
  observe a chain outside its own configured set). Summing per-bridge counts
  can double-count that address.
- **Replacing the global snapshot with a sum would regress today's exact
  answer.** An unfiltered request must stay exact even once bridges overlap;
  it cannot be reimplemented as "sum every bridge's row."

See `.memory-bank/research/stats-subsystem.md` for the endpoint's full
calculation and refresh-flow description, and
`.memory-bank/adr/004-stats-observability-horizon-and-asset-union-find.md`
Decision 5 for the absent-vs-present-but-empty bridge asymmetry this task had
to preserve on the read side.

## Decision

Keep `stats_chains` as the exact, globally deduplicated snapshot and add a
second snapshot, `stats_chains_by_bridge` (`PRIMARY KEY (bridge_id, chain_id)`),
rebuilt by the **same** periodic worker in the **same transaction** as
`stats_chains`. Both tables are always consistent with each other: no reader
can observe one refreshed and the other stale.

`GetChainsStatsRequest` gains `optional string bridge_ids = 12` (shared CSV
filter vocabulary, reusing `parse_bridge_ids_csv`):

- absent/blank -> `stats_chains`, exact, byte-for-byte unchanged from before
  this task;
- one bridge id -> exact per-chain counts for that bridge, including activity
  on chains it reached through `process_unknown_chains`;
- several bridge ids -> **sum of per-bridge distinct counts**, which is exact
  except that it overcounts an address present on the *same chain* through
  more than one selected bridge. This approximation is documented in the
  proto/Swagger contract, not silently accepted.

Visibility and attribution are kept separate on the read side:

- `include_unindexed_chains` keeps its existing **global** meaning — a chain
  is unindexed only when no configured bridge indexes it
  (`IndexedChains::configured_union()`), never "not indexed by the selected
  bridges";
- `IndexedChains::selected_configured_union(bridge_ids)` (new) supplies
  current configured zero-row candidates for the *selected* bridges only,
  returning `Vec<i64>` (never `Option<Vec<i64>>`) so an empty result cannot be
  read as "no restriction" — the opposite convention from
  `configured_union()`;
- historical activity for a bridge removed from runtime config, or one that
  observed an unconfigured chain, stays queryable through
  `stats_chains_by_bridge`, independent of current configuration.

Recomputation derives both snapshots from one bridge-qualified, `MATERIALIZED`
CTE per domain (message, transfer), computed once and shared by the global and
per-bridge aggregation arms — this is also what makes the real cross-bridge
overlap observable without a second full canonical scan. The four canonical
partial indexes that made the old recomputation an index-only scan
(`m20260312_175120_add_stats_tables`) are rebuilt with `bridge_id` as a
trailing key column so the wider projection stays index-only.

Observability: `IndexedChains::configured_overlaps()` (new) warns once at
startup per chain configured by two or more distinct bridge ids — the
*structural* risk. `StatsChainsRecomputeReport` (new), returned by
`recompute_stats_chains`, carries the *actual* cross-bridge overcount per
recomputation; `StatsService::recompute_stats_chains` publishes
`interchain_indexer_stats_chains_bridge_sum_overcount_users` and
`..._affected_chains` gauges (label: `kind` only) after each successful
commit, and a pure `overlap_transition` helper (unit-testable without
Prometheus) drives a one-shot warn-on-appearance / info-on-recovery log
policy. Neither signal substitutes for the other: structural overlap is risk,
the recomputation delta is evidence.

## Alternatives Considered

### Alternative 1 (solution 1): Visibility-only filter over the existing global snapshot

Filter the chain *directory* rows to the selected bridges' configured chains,
but keep reading `unique_transfer_users_count` from the unchanged global
`stats_chains`.

**Pros:**
- No migration, no new table, minimal code change.
- Zero recomputation cost change.

**Cons:**
- Wrong by construction for the stated goal: the returned count would still
  include every other bridge's users on that chain. A single selected bridge
  is not exact, which fails the task's second priority outright.
- `process_unknown_chains` and historical/removed-bridge activity are
  invisible to a purely configuration-driven filter.

Rejected: it does not answer the question the filter exists to answer.

### Alternative 2 (solution 3): Request-time exact `COUNT(DISTINCT ...)` over canonical rows, scoped by `bridge_ids`

Compute the filtered count on demand with a bridge-qualified distinct scan of
`crosschain_messages` / `crosschain_transfers`, instead of a snapshot.

**Pros:**
- Exact for *any* subset of bridges, including multi-bridge selections — no
  additive-overcount caveat at all.
- No new table, no dual-snapshot consistency to maintain.

**Cons:**
- A full distinct scan per request over the two largest tables in the schema,
  for every page of every `/stats/chains` call. Latency and load scale with
  canonical table size, not with response size, which fails the task's
  latency/cost priorities.
- Every request could plan differently depending on the selected bridge set;
  no covering index shape makes an arbitrary bridge subset an index-only scan.

Rejected: violates "keep request-time latency close to the current snapshot
query; avoid a full distinct scan per request," which the confirmed evaluation
criteria ranked above exact arbitrary-subset counting.

### Alternative 3 (solution 4): Persisted user-identity table

Materialize a `(chain_id, bridge_id, address)` (or globally deduplicated
`(chain_id, address, bridge_set)`) identity table that a request-time query
joins/counts against, kept incrementally up to date.

**Pros:**
- Exact for arbitrary bridge subsets without a full canonical scan per
  request.

**Cons:**
- A new high-cardinality table proportional to the number of distinct
  addresses per chain — the largest storage/maintenance commitment of any
  option, for a filter whose accepted approximation (per the confirmed
  evaluation criteria) doesn't require it.
- Incremental maintenance (insert/dedup per canonical row as it is
  processed) is a materially larger, more failure-prone change than a
  periodic full-table rebuild, for a benefit (exact multi-bridge subsets) the
  task explicitly did not require.

Rejected: cost disproportionate to the accepted product need (evaluation
criteria #5).

### Alternative 4: Mergeable sketches (HyperLogLog) per bridge

Store a per-`(bridge_id, chain_id)` HLL sketch and merge the selected bridges'
sketches at request time for an approximate multi-bridge distinct count.

**Pros:**
- Small, roughly constant storage per cell regardless of cardinality.
- Cheap to merge at request time; scales to large bridge-set unions.

**Cons:**
- **Breaks single-bridge exactness.** A single sketch's cardinality estimate
  has statistical error even with no merge involved, which fails the task's
  second, higher-priority requirement ("a single-bridge filtered request is
  exact"). Sketches are the right tool for "many bridges, approximate total,"
  not for "one bridge, exact."
- Estimation error is opaque to the API contract in a way a documented
  additive-overcount rule is not: a caller cannot reason about the bound the
  way they can reason about "same address, same chain, more than one
  selected bridge."

Rejected specifically because it fails single-bridge exactness, which none of
the other rejected alternatives do (they fail on cost or latency instead).

## Consequences

### Positive

- Unfiltered and single-bridge requests are exact, matching the task's top two
  priorities.
- Multi-bridge requests get one documented, bounded approximation instead of
  an undocumented one or a prohibitively expensive exact answer.
- Visibility (`include_unindexed_chains`) and attribution (`bridge_ids`) stay
  orthogonal, so this filter cannot silently reinterpret the existing global
  directory semantics `GetChains` also relies on.
- Two independent signals (structural config overlap at startup; actual
  bridge-sum overcount at each recomputation) give operators warning before
  and evidence after overlap becomes real, without per-request log noise.

### Negative

- A second table to store, index, and rebuild: more storage, more WAL, and a
  wider recomputation query (materialized CTE, two aggregation arms, two
  upserts) than the single-table refresh this replaces.
- The four canonical partial indexes are wider (trailing `bridge_id`), and
  rebuilding them takes an exclusive `CREATE INDEX` lock (no `CONCURRENTLY`
  inside a migration transaction) proportional to `crosschain_messages` /
  `crosschain_transfers` size.
- Multi-bridge selections carry a real, if bounded and documented, overcount.
  Callers that need exact arbitrary-subset counts must not use this endpoint
  for that.

### Neutral

- `stats_chains_by_bridge` is independent of `stats_messages*` /
  `stats_asset_edges` / `bridge_contracts`; nothing about those changes.
- Production-scale recomputation duration, index-rebuild duration, and
  endpoint latency are rollout measurements, not decisions this ADR fixes.

## References

- `.memory-bank/research/stats-subsystem.md`
- `.memory-bank/research/db-schema-and-layer.md`
- `.memory-bank/adr/004-stats-observability-horizon-and-asset-union-find.md` (Decision 5)
- `interchain-indexer-migration/src/m20260824_120000_add_stats_chains_by_bridge.rs`
- `interchain-indexer-logic/src/database.rs` (`recompute_stats_chains`, `StatsChainsRecomputeReport`)
- `interchain-indexer-logic/src/stats_chains_query.rs` (`StatsChainsScope`)
- `interchain-indexer-logic/src/stats/indexed_chains.rs` (`selected_configured_union`, `configured_overlaps`)
- `interchain-indexer-logic/src/stats/overlap_warning.rs`
