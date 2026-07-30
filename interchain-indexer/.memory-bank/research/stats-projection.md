# Stats Projection

## Scope

This note covers how flushed `crosschain_messages` are projected into
`stats_messages` and related aggregate tables — every flushed entry reaches the
projection hook, and a separate eligibility rule decides which of them are
counted — plus how the incremental `stats_processed` marker works and where the
startup backfill path fits.

Since the observability-horizon work (ADR-004), message eligibility is no
longer decided by protocol status alone — it also depends on which chains a
bridge indexes. This note covers that mechanism as it applies to messages;
`stats-subsystem.md` covers the parallel transfer/asset-identity story and the
full API surface. See
`.memory-bank/adr/004-stats-observability-horizon-and-asset-union-find.md` for
the design rationale — this note does not restate it.

## Short Answer

`stats_messages` is not written directly by protocol indexers. Indexers and
buffer maintenance first persist canonical rows into `crosschain_messages` and
`crosschain_transfers`. Stats projection then reads eligible canonical rows,
groups them into aggregate deltas, upserts those deltas into stats tables, and
increments `stats_processed` so the same canonical rows are not counted twice.

Eligibility is **not** "protocol status `Completed`" alone. A message also
counts once its destination chain is no longer indexed by its bridge — because
then the missing confirmation can never arrive, and deferring would discard
the row forever. This is decided by `IndexedChains::may_observe`, the single
per-bridge "can this evidence still arrive?" predicate (`stats/indexed_chains.rs`).

## Why This Matters

Projection is the bridge between canonical interchain storage and the
precomputed directional message stats used by higher-level APIs. If its
eligibility rules, processed markers, or transaction boundaries are wrong, the
system can silently miss counts or double count historical rows. Since
ADR-004, eligibility also depends on live bridge configuration, so the same
canonical row can flip from "deferred" to "countable" without any migration —
getting that wrong either strands data or double-counts it.

## Source-of-Truth Files

- `interchain-indexer-logic/src/stats/projection.rs`
- `interchain-indexer-logic/src/stats/indexed_chains.rs` — `IndexedChains`,
  `message_countable_condition()`, `transfer_identity_ready_condition()`,
  `chain_unindexed_condition()`
- `interchain-indexer-logic/src/stats/metrics.rs`
- `interchain-indexer-logic/src/stats/service.rs`
- `interchain-indexer-logic/src/message_buffer/maintenance.rs`
- `interchain-indexer-logic/src/indexer/avalanche/consolidation.rs`
- `interchain-indexer-server/src/server.rs` — builds `IndexedChains` and wires
  it into `StatsService`
- `interchain-indexer-migration/src/migrations_up/m20260312_175120_add_stats_tables_up.sql`

## Key Types / Tables / Contracts

- `StatsService` — holds an `IndexedChains` alongside the DB handle
- `StatsService::apply_stats_for_flushed_batch(...)` — the live hook (renamed
  from `apply_stats_for_finalized_batch`); see step 2 below
- `project_messages_batch(tx, keys, indexed: &IndexedChains)`
- `project_transfers_batch(tx, transfer_ids, indexed: &IndexedChains)`
- `IndexedChains` — `AllIndexed` (permissive, no config) or
  `PerBridge(HashMap<bridge_id, HashSet<chain_id>>)`
- `finalized_message_stats_condition()` — unchanged: `status = Completed` (any
  bridge) **or** `status = Failed` on an AMB bridge. This is now only the
  *finality* half of eligibility, not the whole gate.
- `message_countable_condition(indexed)` — the actual eligibility gate:
  `finalized_message_stats_condition() OR chain_unindexed_condition(dst
  unindexed for this bridge)`
- `crosschain_messages`
- `stats_messages`
- `stats_messages_days`
- `crosschain_messages.stats_processed`
- `MessageStatus::Completed`

## Step-by-Step Flow

### 1. Flushed rows land in canonical tables

Protocol-specific consolidation builds message and transfer models from whatever
evidence it has. Message-buffer maintenance flushes **every** consolidatable
entry into `crosschain_messages` and `crosschain_transfers` — an entry whose
`is_final` is false is classified `Partial` and flushed all the same. `is_final`
governs pending-tier cleanup, hot-tier eviction and finalized-batch metrics; it
has never governed whether a row is persisted, and since ADR-004 it does not
govern whether a row reaches the projection hook either.

Primary code paths:

- message creation:
  `interchain-indexer-logic/src/indexer/avalanche/consolidation.rs`
- canonical persistence and maintenance orchestration:
  `interchain-indexer-logic/src/message_buffer/maintenance.rs`
- canonical persistence helpers:
  `interchain-indexer-logic/src/message_buffer/persistence.rs`

### 2. Every flushed entry reaches the stats hook — not only final ones

`commit_maintenance` passes **all** flushed entries for the cycle — final and
`Partial` alike — to `StatsService::apply_stats_for_flushed_batch(...)`, with
no `is_final` filter. This is deliberate: asset/token identity maintenance
(see `stats-subsystem.md`) must run for every flushed canonical key, including
a `Partial` entry that only just filled in a previously-missing token address,
or a later relink would be silently missed. `is_final` still governs whether
the entry is evicted from the hot buffer, removed from `pending_messages`, and
counted in finalized-batch metrics — none of that is stats-projection's
concern.

Actual **counting** is still gated inside the projection functions themselves,
not by the hook: `project_messages_batch` only increments `stats_processed`
for rows matching `stats_processed = 0 AND message_countable_condition(indexed)`.

### 3. Stats projection runs from canonical rows, gated by observability

`stats_messages` is a bridge-qualified directional aggregate keyed by:

- `bridge_id`
- `src_chain_id`
- `dst_chain_id`

Each row stores a count of finalized-or-unconfirmable messages for that
`(bridge, src, dst)` edge — "finalized-or-unconfirmable" because a message
also counts once its destination chain is no longer indexed for that bridge,
even while `status = Initiated`. The same directional chain edge on two
different bridges is two distinct rows; read queries that do not filter by
bridge `SUM` over the bridge dimension to reproduce the bridge-collapsed
totals.

Related tables (all three additive aggregates are bridge-qualified since
`m20260720_120000_add_read_filters_and_bridge_stats`):

- `stats_messages_days` — keyed by `(date, bridge_id, src_chain_id,
  dst_chain_id)`, the same directional count split by day
- `stats_asset_edges` — keyed by `(stats_asset_id, bridge_id, src_chain_id,
  dst_chain_id)`; `stats_assets` / `stats_asset_tokens` stay global (only the
  movement/count edges gain the bridge dimension)

The schema is introduced in:

- `interchain-indexer-migration/src/migrations_up/m20260312_175120_add_stats_tables_up.sql`

### 4. Each projection batch reloads and filters canonical messages

In the same maintenance transaction, `project_messages_batch(...)` reloads the
canonical message rows for the flushed primary keys and filters to rows that
are:

- `stats_processed = 0`
- eligible per `message_countable_condition(indexed)` — either
  `finalized_message_stats_condition()` (`status = completed` on any bridge,
  or `status = failed` on an AMB bridge), **or** the destination chain is not
  indexed by this bridge (`chain_unindexed_condition`, built from
  `IndexedChains::may_observe`)
- `dst_chain_id IS NOT NULL` (a NULL destination can never satisfy either
  branch — there is no chain to test unindexed-ness against, so it always
  defers)

`transfer_identity_ready_condition(indexed)` is the transfer-side analogue,
used by `project_transfers_batch`; see `stats-subsystem.md` for the fuller
transfer/asset story.

Primary code paths:

- projection implementation:
  `interchain-indexer-logic/src/stats/projection.rs`
- eligibility predicates:
  `interchain-indexer-logic/src/stats/indexed_chains.rs`

### 5. Projection groups eligible rows into aggregate deltas

Eligible rows are grouped by bridge-qualified directional edge —
`(bridge_id, src_chain_id, dst_chain_id)` and, for the daily table,
`(date, bridge_id, src_chain_id, dst_chain_id)`. Projection
then upserts those deltas into `stats_messages` and `stats_messages_days`, and
increments `crosschain_messages.stats_processed` for the counted rows.

### 6. Startup backfill reuses the same eligibility rules, in two sequential phases

There is also a startup backfill path for historical rows:

- when `stats.backfill_on_start = true` (env:
  `INTERCHAIN_INDEXER__STATS__BACKFILL_ON_START=true`), server startup triggers
  a stats backfill pass
- `backfill_stats_until_idle_with_token_enrichment()` (`database.rs`) drives
  **two sequential phases**, each with its own **monotonic id cursor**: a
  message-candidate phase (`message_min_id`) run to idle first, then a
  transfer-candidate phase (`transfer_min_id`). This ordering is required, not
  cosmetic — the transfer candidate query requires the parent message's
  `stats_processed > 0`, so interleaving the two cursors could permanently
  skip a low-id transfer whose parent message has a high id.
- each round (`backfill_stats_projection_round`) filters candidates through
  the **same** `message_countable_condition(indexed)` /
  `transfer_identity_ready_condition(indexed)` used by live projection, so live
  and backfill can never diverge on what counts as eligible
- each phase's loop breaks on **candidates scanned == 0**, not on rows
  actually processed/counted == 0. This is deliberate: because the candidate
  queries and the projection functions share the same condition builders,
  `processed == 0 ⟺ scanned == 0` by construction; breaking on `scanned == 0`
  stays safe even if that invariant ever drifts (a stale hand-copied predicate
  would otherwise leave a silent backlog), and the monotonic `min_id` cursor
  bounds the loop by the id space regardless of which counter gates the break.

Primary code paths:

- startup trigger:
  `interchain-indexer-server/src/server.rs`
- service orchestration:
  `interchain-indexer-logic/src/stats/service.rs`
- backfill round + drive loop: `interchain-indexer-logic/src/database.rs`
  (`backfill_stats_projection_round`, `backfill_stats_until_idle_with_token_enrichment`)

Ordering note:

- startup backfill is intentionally executed before `upsert_chains`,
  `upsert_bridges`, and `upsert_bridge_contracts` — this is now load-bearing
  for `IndexedChains` too, not just for reference-data freshness:
  `IndexedChains` is built once from the in-memory `bridges.json` config
  (`IndexedChains::from_bridges` in `server.rs`), **never** from the
  `bridge_contracts` table, precisely because that table has no rows yet
  (or stale ones) exactly when backfill needs an accurate set.

### 7. Queries read the aggregate tables, with clear limits

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
- `stats_processed` is the guard against double counting, and is **additive,
  exactly once, never reset, never reversed** — it guards counting only, not
  asset/token identity maintenance (see `stats-subsystem.md`)
- a message row is counted only when it is in the projection batch,
  `stats_processed = 0`, `dst_chain_id` is not null, and
  `message_countable_condition(indexed)` holds — i.e. either the shared
  finality predicate (completed on any bridge, or failed on an AMB bridge) or
  its destination chain is not indexed by this bridge
- the three additive aggregates are bridge-qualified; projection never merges
  identical edges from different bridges, and it sets `bridge_id` in every active
  model / `ON CONFLICT` target / exact-row update (message counts and
  `stats_asset_edges`, including token-metadata propagation)
- projection is batch-oriented and transaction-scoped (aggregate deltas and the
  matching `stats_processed` increment commit together, so a crash is safe to
  resume)
- the startup backfill path applies the same eligibility and aggregation rules
  as the maintenance-triggered projection path (`message_countable_condition`
  is the single shared gate for both)
- the eligibility gate depends on **live, in-memory** bridge configuration,
  never on `bridge_contracts`; a bridge removed from config stays permissive
  (`may_observe` defaults to `true` for an absent bridge) so its already-
  counted history is never retroactively reinterpreted — see ADR-004
  Decision 5 and `gotchas.md`
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
- `STATS_INDEXED_CHAINS` (gauge, per bridge) is published once at startup from
  `IndexedChains::record_metrics()` — a bridge showing `0` is either absent
  from the metric entirely (never in the map) or present with zero chains (a
  misconfigured, contract-less bridge); check startup warn logs to tell which
- `STATS_TRANSFERS_DEFERRED_TOTAL{reason}` (events, not distinct rows) counts
  deferral decisions on the transfer path; see `stats-subsystem.md` for the
  full metric set

Primary places to inspect:

- startup logs for backfill activity and per-bridge `STATS_INDEXED_CHAINS`
- buffer maintenance logs, since live projection runs inside maintenance
- `crosschain_messages.stats_processed` when checking whether rows were
  projected
- `stats_messages` and `stats_messages_days` contents for directional totals
- `.memory-bank/runbooks/runtime-verification.md` for copy-paste-runnable,
  read-only SQL to check these invariants against a live database

## Edge Cases / Gotchas

- a message can exist canonically without being counted yet if maintenance or
  backfill has not projected it
- startup backfill is useful after introducing new stats tables for existing
  data
- message counts are directional; `A -> B` and `B -> A` are different rows
- stats are near-realtime, not immediate: messages must reach repo-specific
  finality (or have their destination chain drop out of the indexed set),
  then be flushed by buffer maintenance, and only then can projection
  increment aggregate tables
- a transfer counted because its destination chain was unindexed stays
  counted after that chain is later added to the bridge's config, even if the
  movement never actually completed — an accepted inaccuracy confined to the
  opt-in unindexed slice (see `stats-subsystem.md`'s read-filter section), not
  a bug
- `interchain-indexer-logic/src/database.rs` contains lower-level stats helper
  methods (including the backfill round/drive functions), but the
  authoritative production semantics for message counts are in
  `interchain-indexer-logic/src/stats/projection.rs` and
  `interchain-indexer-logic/src/stats/indexed_chains.rs`

## Change Triggers

Update this note when:

- message eligibility rules for projection change (including
  `IndexedChains`/`message_countable_condition` semantics)
- `stats_processed` semantics change
- `stats_messages` or `stats_messages_days` schema changes
- startup backfill behavior changes, including the two-phase cursor structure
  or the scanned-vs-processed loop-exit decision
- directional message-path APIs stop reading these projected tables

## Open Questions

- whether some lower-level stats helper paths should be documented separately if
  they become production-relevant
