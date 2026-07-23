# Stats Projection

## Scope

This note covers how finalized `crosschain_messages` are projected into
`stats_messages` and related aggregate tables, how the incremental
`stats_processed` marker works, and where the startup backfill path fits.

For the broader stats API surface, datasource split, and refresh models across
the whole embedded stats subsystem, see `stats-subsystem.md`.

## Short Answer

`stats_messages` is not written directly by protocol indexers. Indexers and
buffer maintenance first persist canonical rows into `crosschain_messages` and
`crosschain_transfers`. Stats projection then reads eligible canonical rows,
groups them into aggregate deltas, upserts those deltas into stats tables, and
increments `stats_processed` so the same canonical rows are not counted twice.

## Why This Matters

Projection is the bridge between canonical interchain storage and the
precomputed directional message stats used by higher-level APIs. If its
eligibility rules, processed markers, or transaction boundaries are wrong, the
system can silently miss counts or double count historical rows.

## Source-of-Truth Files

- `interchain-indexer-logic/src/stats/projection.rs`
- `interchain-indexer-logic/src/stats/service.rs`
- `interchain-indexer-logic/src/message_buffer/maintenance.rs`
- `interchain-indexer-logic/src/message_buffer/persistence.rs`
- `interchain-indexer-logic/src/indexer/avalanche/consolidation.rs`
- `interchain-indexer-server/src/server.rs`
- `interchain-indexer-migration/src/migrations_up/m20260312_175120_add_stats_tables_up.sql`

## Key Types / Tables / Contracts

- `StatsService`
- `project_messages_batch(...)`
- `crosschain_messages`
- `stats_messages`
- `stats_messages_days`
- `crosschain_messages.stats_processed`
- `MessageStatus::Completed`

## Step-by-Step Flow

### 1. Finalized rows land in canonical tables

Protocol-specific consolidation creates finalized message and transfer models.
Message-buffer maintenance then flushes those finalized rows into
`crosschain_messages` and `crosschain_transfers`.

Primary code paths:

- finalized message creation:
  `interchain-indexer-logic/src/indexer/avalanche/consolidation.rs`
- canonical persistence and maintenance orchestration:
  `interchain-indexer-logic/src/message_buffer/maintenance.rs`
- canonical persistence helpers:
  `interchain-indexer-logic/src/message_buffer/persistence.rs`

### 2. Stats projection runs from canonical rows

`stats_messages` is not written directly by protocol indexers. Instead, stats
projection reads canonical `crosschain_messages` rows and projects eligible
rows into aggregate tables.

`stats_messages` is a bridge-qualified directional aggregate keyed by:

- `bridge_id`
- `src_chain_id`
- `dst_chain_id`

Each row stores a count of finalized messages for that `(bridge, src, dst)`
edge. The same directional chain edge on two different bridges is two distinct
rows; read queries that do not filter by bridge `SUM` over the bridge dimension
to reproduce the bridge-collapsed totals.

Related tables (all three additive aggregates are bridge-qualified since
`m20260720_120000_add_read_filters_and_bridge_stats`):

- `stats_messages_days` — keyed by `(date, bridge_id, src_chain_id,
  dst_chain_id)`, the same directional count split by day
- `stats_asset_edges` — keyed by `(stats_asset_id, bridge_id, src_chain_id,
  dst_chain_id)`; `stats_assets` / `stats_asset_tokens` stay global (only the
  movement/count edges gain the bridge dimension)

The schema is introduced in:

- `interchain-indexer-migration/src/migrations_up/m20260312_175120_add_stats_tables_up.sql`

### 3. Each projection batch reloads and filters canonical messages

In the same maintenance transaction, stats projection runs for the flushed
batch. `project_messages_batch(...)` reloads the canonical message rows for the
flushed primary keys and filters to rows that are:

- `stats_processed = 0`
- eligible per the shared finality predicate: `status = completed` (any bridge)
  **or** `status = failed` on an AMB bridge — `finalized_message_stats_condition()`
  in `stats/projection.rs`, exposed `pub(crate)` and reused by the startup
  backfill candidate queries in `database.rs` so live and backfill can never
  diverge
- `dst_chain_id IS NOT NULL`

Primary code paths:

- projection implementation:
  `interchain-indexer-logic/src/stats/projection.rs`

### 4. Projection groups eligible rows into aggregate deltas

Eligible rows are grouped by bridge-qualified directional edge —
`(bridge_id, src_chain_id, dst_chain_id)` and, for the daily table,
`(date, bridge_id, src_chain_id, dst_chain_id)`. Projection
then upserts those deltas into `stats_messages` and `stats_messages_days`, and
increments `crosschain_messages.stats_processed` for the counted rows.

### 5. Startup backfill reuses the same projection rules

There is also a startup backfill path for historical rows:

- when `stats.backfill_on_start = true` (env:
  `INTERCHAIN_INDEXER__STATS__BACKFILL_ON_START=true`), server startup triggers
  a stats backfill pass
- the backfill scans canonical rows with `stats_processed = 0`
- it applies the same projection logic in batches until no eligible rows remain

Primary code paths:

- startup trigger:
  `interchain-indexer-server/src/server.rs`
- service orchestration:
  `interchain-indexer-logic/src/stats/service.rs`

Ordering note:

- startup backfill is intentionally executed before `upsert_chains`,
  `upsert_bridges`, and `upsert_bridge_contracts` so historical rows are
  projected against the same reference data they were indexed with.

### 6. Queries read the aggregate tables, with clear limits

`stats_messages` is well-suited for:

- total messages from chain A to chain B
- total outbound messages per source chain
- total inbound messages per destination chain
- top directional edges by message volume
- graph-like directional traffic views
- per-bridge directional counts (filter by `bridge_id`; unfiltered reads `SUM`
  over the bridge dimension to reproduce bridge-collapsed totals)

`stats_messages` alone does not answer:

- time-series beyond the available day bucket table
- unique user counts
- protocol-segmented counts
- initiated vs completed vs failed breakdowns
- latency metrics
- token value / volume questions

Those require either canonical-table queries or additional stats tables.

## Invariants

- stats are derived from canonical tables, not raw logs
- `stats_processed` is the guard against double counting
- a message row is counted only when it is in the projection batch,
  `stats_processed = 0`, eligible (completed on any bridge, or failed on an AMB
  bridge), and `dst_chain_id` is not null
- the three additive aggregates are bridge-qualified; projection never merges
  identical edges from different bridges, and it sets `bridge_id` in every active
  model / `ON CONFLICT` target / exact-row update (message counts and
  `stats_asset_edges`, including token-metadata propagation)
- projection is batch-oriented and transaction-scoped (aggregate deltas and the
  matching `stats_processed` increment commit together, so a crash is safe to
  resume)
- the startup backfill path applies the same eligibility and aggregation rules
  as the maintenance-triggered projection path (one shared finality predicate)
- **projection-invalidating migrations** (e.g. the bridge-qualified rebuild) are
  atomic: they clear the three aggregates and reset `stats_processed` for both
  canonical tables together, then rely on `BACKFILL_ON_START=true` to rebuild
  the projections. Never clear the aggregates without resetting the markers
  (loses stats) or reset the markers without clearing (double counts). See the
  README "Stats projection maintenance rebuilds" runbook.

## Failure Modes / Observability

- canonical messages can exist without corresponding `stats_messages*` rows yet
  if maintenance or backfill has not projected them
- incorrect `stats_processed` handling can lead to missed counts or double
  counting
- startup backfill can leave historical directional stats incomplete if it is
  not run after introducing stats tables on a populated database
- because projection runs after canonical persistence, directional message
  stats are near-realtime rather than immediate on raw event ingestion

Primary places to inspect:

- startup logs for backfill activity
- buffer maintenance logs, since live projection runs inside maintenance
- `crosschain_messages.stats_processed` when checking whether rows were
  projected
- `stats_messages` and `stats_messages_days` contents for directional totals

## Edge Cases / Gotchas

- a message can exist canonically without being counted yet if maintenance or
  backfill has not projected it
- startup backfill is useful after introducing new stats tables for existing
  data
- message counts are directional; `A -> B` and `B -> A` are different rows
- stats are near-realtime, not immediate: messages must reach repo-specific
  finality, then be flushed by buffer maintenance, and only then can projection
  increment aggregate tables
- `interchain-indexer-logic/src/database.rs` contains lower-level stats helper
  methods, but the authoritative production semantics for message counts are in
  `interchain-indexer-logic/src/stats/projection.rs`

## Change Triggers

Update this note when:

- message eligibility rules for projection change
- `stats_processed` semantics change
- `stats_messages` or `stats_messages_days` schema changes
- startup backfill behavior changes
- directional message-path APIs stop reading these projected tables

## Open Questions

- whether some lower-level stats helper paths should be documented separately if
  they become production-relevant
