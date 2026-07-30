# Interchain Stats Subsystem

## Scope

This note covers the embedded stats subsystem inside `interchain-indexer`:

- supported stats API endpoints
- the underlying stats tables and query paths
- how values are calculated
- which endpoints read live canonical data vs precomputed stats
- refresh and backfill behavior
- the observability-horizon eligibility rule and the opt-in unindexed-chain
  read filter that exposes it (ADR-004)
- asset identity as a union-find merge, and how counting and identity
  maintenance are kept separate

This note does not cover the standalone external stats service in detail, and
it does not restate ADR-004's reasoning — see
`.memory-bank/adr/004-stats-observability-horizon-and-asset-union-find.md` for
that; this note describes current behavior and points there for "why." For
copy-paste-runnable SQL to confirm this behavior against a live database, see
`.memory-bank/runbooks/runtime-verification.md`.

## Short Answer

`interchain-indexer` contains an embedded stats subsystem because some
interchain analytics need domain-specific read models that are not well
represented as simple counters or line-chart series. In this repo, those read
models are materialized into `stats_*` tables and exposed via dedicated stats
endpoints.

The subsystem is not uniform. It has three refresh modes:

- direct request-time queries over canonical tables
- incremental projection into `stats_*` tables on every flushed batch, with
  countability decided per row rather than by the flush being final
- periodic full recomputation for per-chain user counters

Backfill does not use separate calculation logic. It reuses the same
eligibility predicates and projection functions as the live incremental path,
run in two sequential phases (messages, then transfers) each with its own
monotonic id cursor.

Since ADR-004, eligibility for all three additive aggregates
(`stats_messages`, `stats_messages_days`, `stats_asset_edges`) is decided by
one protocol-agnostic question — *can the missing evidence still arrive?* —
answered per bridge by `IndexedChains::may_observe`. A message or transfer
whose missing side sits on a chain no longer indexed by its bridge is counted
now instead of deferred forever; that same test also drives an opt-in
`include_unindexed_chains` read filter, so clients can include or exclude that
slice.

Asset identity (`stats_assets` / `stats_asset_tokens`) is resolved as a
union-find problem with eager merge, not by refusing on any two-different-
assets conflict; the only remaining refusal is two different tokens landing on
one chain inside the same asset.

The stats API surface also mixes two output shapes:

- simple request-time counters over canonical data
- richer interchain-specific read models such as bridged-token tables,
  directional message paths, and ranked chain stats

That split is useful for understanding why this subsystem exists even though
not every `/stats/*` endpoint is backed by a durable projected table.

For the deeper runtime semantics of incremental directional message projection,
processed markers, and startup catch-up for `stats_messages*`, see
`stats-projection.md`.

## Why This Matters

This subsystem is easy to misunderstand because `/stats/*` endpoints do not all
behave the same way:

- some endpoints are backed by precomputed tables
- some are live scans over canonical tables
- some refresh on every flushed batch, final or `Partial`
- one refreshes on a background period
- since ADR-004, "counted" no longer means "protocol-complete" — it can also
  mean "protocol-incomplete but permanently unconfirmable," a distinction that
  only the opt-in filter and `has_unindexed_chain` flag surface to clients

Operationally, this affects:

- query cost
- freshness expectations
- whether a value can lag after indexing
- whether startup backfill is needed
- how to recover after schema or projection changes
- whether a given row reflects confirmed activity or the accepted
  unindexed-chain approximation

## Source-of-Truth Files

- `interchain-indexer-proto/proto/v1/stats.proto`
- `interchain-indexer-proto/proto/v1/interchain_indexer.proto`
- `interchain-indexer-proto/proto/v1/api_config_http.yaml`
- `interchain-indexer-server/src/services/stats.rs`
- `interchain-indexer-server/src/services/interchain_service.rs`
- `interchain-indexer-server/src/services/bridge_proto.rs` — builds the
  `Bridge` proto message, including `indexed_chain_ids`
- `interchain-indexer-server/src/services/utils.rs`
- `interchain-indexer-server/src/server.rs`
- `interchain-indexer-server/src/settings.rs`
- `interchain-indexer-logic/src/stats/service.rs`
- `interchain-indexer-logic/src/stats/projection.rs`
- `interchain-indexer-logic/src/stats/indexed_chains.rs` — `IndexedChains`,
  the shared eligibility/read-filter predicate
- `interchain-indexer-logic/src/stats/metrics.rs`
- `interchain-indexer-logic/src/filters.rs` — `ChainBridgeFilter`, the
  read-side SeaORM condition builder consuming `IndexedChains`
- `interchain-indexer-logic/src/database.rs`
- `interchain-indexer-logic/src/bridged_tokens_query.rs`
- `interchain-indexer-logic/src/stats_chains_query.rs`
- `interchain-indexer-logic/src/message_buffer/maintenance.rs`
- `interchain-indexer-logic/src/settings.rs`
- `interchain-indexer-migration/src/migrations_up/m20260312_175120_add_stats_tables_up.sql`

## Key Types / Tables / Contracts

### API contracts

- `InterchainStatisticsService`
- `GetCommonStatistics*`
- `GetDailyStatistics*`
- `GetBridgedTokens*`
- `GetChainsStats*`
- `GetMessagePaths*`
- `InterchainService::GetChains*`, `GetBridges*` (`interchain_indexer.proto`) —
  directory endpoints that also carry the observability-horizon flags

### Core service / orchestration types

- `StatsService` — now also holds an `IndexedChains`
- `BridgedTokenListRow`
- `StatsChainListRow`
- `IndexedChains` — `AllIndexed` | `PerBridge(HashMap<bridge_id, HashSet<chain_id>>)`

### Stats tables

- `stats_assets`
- `stats_asset_tokens`
- `stats_asset_edges`
- `stats_chains`
- `stats_messages`
- `stats_messages_days`

### Canonical-table incremental markers

- `crosschain_messages.stats_processed`
- `crosschain_transfers.stats_processed`
- `crosschain_transfers.stats_asset_id`

## Subsystem Boundary

Code-derived fact:

- this repo owns domain-specific interchain stats tables and stats endpoints
- those endpoints include bridged tokens, chain stats, and message paths

User-provided product context:

- the embedded subsystem complements a separate standalone stats service
- the main reason this subsystem exists is that some interchain stats outputs
  do not fit a generic counters / line-chart model

That product framing matches the code structure here: the repo contains both
simple counter-style endpoints and richer interchain-specific read models that
need dedicated tables, joins, pagination, and directional or asset-aware
aggregation rules.

## Supported API Endpoints

Defined in `stats.proto` and HTTP-mapped in `api_config_http.yaml`:

- `/api/v1/stats/common`
- `/api/v1/stats/daily`
- `/api/v1/stats/chain/{chain_id}/bridged-tokens`
- `/api/v1/stats/chains`
- `/api/v1/stats/chain/{chain_id}/messages-paths/sent`
- `/api/v1/stats/chain/{chain_id}/messages-paths/received`

Adjacent directory endpoints in `interchain_indexer.proto` also participate in
the observability-horizon model even though they are not `/stats/*`:

- `GetChains` — chain directory; filtered by `IndexedChains::configured_union()`
- `GetBridges` — bridge directory; each `Bridge` now carries
  `indexed_chain_ids` (`services/bridge_proto.rs`)
- `GetMessages*`, `GetTransfers*` and their `byTransaction`/`byAddress`
  variants — canonical list endpoints; each accepts `include_unindexed_chains`
  and each returned message/transfer carries `has_unindexed_chain`

## Data Sources

The embedded stats subsystem uses four effective data-source patterns.

### 1. Canonical interchain tables

Used directly by request-time queries:

- `crosschain_messages`
- `crosschain_transfers`

### 2. Projected message stats tables

Materialized from canonical messages once countable (finalized, **or**
permanently unconfirmable because the destination chain is no longer indexed
for that bridge — see "Observability Horizon" below):

- `stats_messages`
- `stats_messages_days`

### 3. Projected asset/token stats tables

Materialized from canonical transfers once their identity is resolved and,
separately, once they are countable:

- `stats_assets`
- `stats_asset_tokens`
- `stats_asset_edges`

### 4. Periodic snapshot table

Rebuilt from canonical tables:

- `stats_chains`

## Observability Horizon: The Eligibility Rule (ADR-004)

The single question the stats layer asks, for messages and transfers alike:
*can the missing evidence still arrive?* It can exactly when the chain that
would produce it is indexed **by that bridge** (a configured contract there).
`IndexedChains::may_observe(bridge_id, chain_id)` (`stats/indexed_chains.rs`)
is the one method that answers it, used identically by projection eligibility
and by the read-side unindexed-chain filter — there is no second predicate to
drift out of sync.

- missing token endpoint, counterpart chain indexed → defer
- missing token endpoint, counterpart chain unindexed → commit to what is known
- missing destination confirmation, destination chain indexed → defer
- missing destination confirmation, destination chain unindexed → count now

`IndexedChains` is built once at startup from the **in-memory** `bridges.json`
config (`IndexedChains::from_bridges` in `server.rs`), never from the
`bridge_contracts` table — that table is stale exactly when startup backfill
needs an accurate set (backfill runs before `upsert_bridge_contracts`). A
bridge *absent* from the config is permissive (`may_observe` returns `true`,
so its already-indexed history is never retroactively reinterpreted); a
bridge *present with zero contracts* is restrictive (misconfiguration,
surfaced by a startup warning). See `gotchas.md`, "Stats Eligibility Is About
Observability, Not Protocol Terminality," for the full asymmetry and its
consequences, and ADR-004 Decision 5 for the rationale.

Two Rust-level condition builders spell this out for SQL:

- `message_countable_condition(indexed)` — messages/message-paths
- `transfer_identity_ready_condition(indexed)` — transfer identity resolution

Both are shared verbatim between live projection (`stats/projection.rs`) and
startup backfill (`database.rs`), so the two paths cannot diverge on what
counts as eligible.

### Read-side counterpart: the unindexed-chain filter

The same `may_observe` predicate drives an opt-in read filter:

- `include_unindexed_chains` (request field) — when `false` (default), list
  and stats endpoints exclude rows whose bridge could not have fully observed
  both endpoints; when `true`, those rows are included too
- `has_unindexed_chain` (response field, always present) — set on
  `InterchainMessage` and `InterchainTransfer` so a client consuming the
  widened view can still tell which rows are the approximation
- `indexed_chain_ids` (response field on `Bridge`) — the bridge's actual
  configured chain set, from `IndexedChains::chain_ids_for(bridge_id)`, never
  from `bridge_contracts`

`ChainBridgeFilter` (`filters.rs`) renders `only_indexed_by_bridge` for the
canonical list endpoints (`GetMessages*`, `GetTransfers*`); the raw-SQL stats
endpoints (bridged-tokens, message-paths) render the equivalent restriction
via `push_indexed_pairs_predicate` in `database.rs`. Both consume
`IndexedChains::configured_pairs(...)` — the same per-bridge pair data, two
renderers.

`/stats/chains` and `GetChains` are the two exceptions: both are chain
*directories*, not bridge-qualified rows, so they use
`IndexedChains::configured_union()` — the union of every bridge's indexed set
— rather than a per-bridge restriction. The same accessor backs both, so the
two directory views cannot drift apart.

## Asset Identity: Union-Find With Eager Merge (ADR-004)

`ensure_asset_for_transfer` / `merge_assets` (`stats/projection.rs`) resolve
each transfer's two token endpoints as a union-find problem: an asset is a
connected component of chain-local tokens, and a transfer is an edge. When both
endpoints already map to two *different* existing assets, the components are
merged rather than the transfer being skipped:

- winner = the component with more linked tokens, ties broken by lower id
- mutation order is fixed (tokens → edges → transfers → metadata → delete)
  because FK cascades make an early delete a silent data-loss bug
- edge folding rescales the loser's `cumulative_amount` to the winner's
  decimals rather than refusing (`scaled_up`/`scaled_down`/
  `unscaled_unknown_decimals`/`unscaled_overflow`, per
  `STATS_EDGE_RESCALED_FOLD_TOTAL`); a mixed `amount_side` between the two
  components is summed anyway and flagged via
  `STATS_EDGE_MIXED_AMOUNT_SIDE_TOTAL`
- the only genuine refusal left: a merge that would place two different
  tokens of one chain into a single asset (a `stats_asset` can hold at most
  one token per chain) — `STATS_ASSET_MERGES_TOTAL{outcome="refused_chain_collision"}`

This replaces the old permanent `warn + skip` on any two-different-assets
conflict, which could never resolve a genuine three-or-more-chain asset split.
See `gotchas.md`, "Stats Asset Mapping Conflicts Merge; Only Same-Chain
Collisions Skip," for the operational read of this behavior, and ADR-004
Decision 2 for the design.

### Counting and identity are separate concerns

`stats_processed` guards **counting** only — additive, exactly once, never
reversed. Asset/token **identity** maintenance (linking a newly-known endpoint,
merging two components) is idempotent and may re-run for an already-counted
transfer. This is why `StatsService::apply_stats_for_flushed_batch` (renamed
from `apply_stats_for_finalized_batch`) takes **every** flushed entry, final
and `Partial` — identity maintenance must see every flushed canonical key, not
only finalized ones, or a later relink (a flush that fills in a previously
missing token address) would be silently missed. Actual counting stays gated
inside `project_messages_batch`/`project_transfers_batch` on
`stats_processed = 0` plus eligibility, so widening the trigger did not widen
what gets counted.

### Decimals conflict on the counting path — a separate, non-corrupting skip

Distinct from the union-find merge conflict: a transfer whose asset identity
already resolved (directly or via merge) can still hit a **decimals conflict**
on the edge-accumulation step — a token's decimals value differs from what an
existing edge already recorded. This transfer's amount is skipped from
`stats_asset_edges` (`STATS_EDGE_DECIMALS_CONFLICT_TOTAL`), but:

- the transaction still commits — a decimals conflict is not an abort
- `stats_processed` still increments for the transfer
- `stats_asset_id` is still **set** to the resolved asset (not left `NULL`)

Read `crosschain_transfers.stats_asset_id` accordingly: `NULL` means identity
is genuinely unknown or ambiguous (the chain-collision merge refusal is the
only remaining case); a set `stats_asset_id` with no corresponding edge
contribution means identity is known but this transfer's amount specifically
was not counted (the decimals-conflict case). See `gotchas.md` for the full
read.

## Endpoint Matrix

| Endpoint | Data source | Freshness model | Refresh trigger | Configurable period |
| --- | --- | --- | --- | --- |
| `/api/v1/stats/common` | Direct query over `crosschain_messages` + `crosschain_transfers` | Request-time live DB read | Every request | No |
| `/api/v1/stats/daily` | Direct query over `crosschain_messages` + `crosschain_transfers` | Request-time live DB read | Every request | No |
| `/api/v1/stats/chain/{chain_id}/bridged-tokens` | `stats_asset_edges` + `stats_assets` + `stats_asset_tokens` + `tokens` | Pre-calculated, near-realtime | Projection during flushed batch (final or `Partial`) | Indirectly via buffer maintenance interval |
| `/api/v1/stats/chain/{chain_id}/messages-paths/sent` | `stats_messages` or `stats_messages_days` | Pre-calculated, near-realtime | Projection during flushed batch | Indirectly via buffer maintenance interval |
| `/api/v1/stats/chain/{chain_id}/messages-paths/received` | `stats_messages` or `stats_messages_days` | Pre-calculated, near-realtime | Projection during flushed batch | Indirectly via buffer maintenance interval |
| `/api/v1/stats/chains` | `chains LEFT JOIN stats_chains` | Pre-calculated periodic snapshot | Background full recomputation worker | Yes |

## Step-by-Step Flow

### 1. Canonical rows are persisted first

The indexer and message buffer persist canonical rows into:

- `crosschain_messages`
- `crosschain_transfers`

Stats are downstream of canonical persistence. They are not direct side effects
of raw event handling.

### 2. Every flushed batch reaches the stats hook; eligibility decides what's counted

When message-buffer maintenance flushes a batch (final and `Partial` entries
alike), it calls `StatsService::apply_stats_for_flushed_batch(...)` inside the
same DB transaction.

That method:

- collects the flushed message primary keys
- calls `project_messages_batch(...)` with `IndexedChains`
- finds every transfer belonging to those keys (no `stats_processed = 0`
  pre-filter — an already-counted transfer must still reach identity
  maintenance)
- calls `project_transfers_batch(...)` with the same `IndexedChains`

Both projection functions internally decide, per row, whether it is eligible
for **counting** (via `message_countable_condition`/
`transfer_identity_ready_condition`) versus only eligible for **identity
maintenance** (already `stats_processed > 0`). This is the main near-realtime
stats path.

For detailed message-projection semantics, see `stats-projection.md`.

### 3. Startup backfill reuses the same eligibility rules, in two phases

When `stats.backfill_on_start = true` (env:
`INTERCHAIN_INDEXER__STATS__BACKFILL_ON_START=true`), startup calls
`backfill_stats_until_idle_with_token_enrichment()`.

That method runs a message-candidate phase to idle (its own monotonic
`message_min_id` cursor), then a transfer-candidate phase to idle (its own
`transfer_min_id` cursor) — never interleaved, because the transfer query
depends on the parent message already being `stats_processed > 0`. Each
round's candidate query filters through the same `message_countable_condition`
/ `transfer_identity_ready_condition` used by live projection, and each phase's
loop exits when **candidates scanned** (not rows processed) reaches zero —
see `stats-projection.md` for why that is the correct, deliberate condition.

Backfill is therefore a catch-up wrapper around projection, not separate stats
logic.

For the detailed relationship between startup backfill and projection
eligibility rules, see `stats-projection.md`.

### 4. `stats_chains` is refreshed separately

`stats_chains` does not use the incremental finalized-batch projection path.
Instead, a background worker periodically runs `recompute_stats_chains()`, which
rebuilds the table from canonical messages and transfers. It has no bridge
dimension and is not affected by `IndexedChains` — see "Read-Time
Filterability Constraints" below.

### 5. Some endpoints bypass derived stats tables entirely

`/stats/common` and `/stats/daily` query canonical tables directly on every
request.

Code-derived fact:

- they do not read `stats_*` aggregate tables

User/product context:

- these are early POC-style endpoints and are considered inefficient on large
  datasets

## Endpoint-by-Endpoint Calculation Rules

### `/stats/common`

Source:

- `crosschain_messages`
- `crosschain_transfers` joined through messages

Calculation:

- build a message filter using `init_timestamp < timestamp`
- optionally apply source and destination chain filters at DB-layer helpers
- count matching message rows
- count matching transfer rows through the message join

Properties:

- request-time query
- no precomputation
- no recalculation period

### `/stats/daily`

Source:

- `crosschain_messages`
- `crosschain_transfers`

Calculation:

- derive the UTC day from the request timestamp
- filter messages where `init_timestamp` falls within `[day_start, next_day_start)`
- count distinct message primary keys
- count total joined transfers

Properties:

- request-time query
- no precomputation
- no recalculation period

### `/stats/chain/{chain_id}/messages-paths/sent`

All-time source:

- `stats_messages`

Bounded-date source:

- `stats_messages_days`

Calculation:

- sent: filter `src_chain_id = chain_id`
- optionally filter destination counterparties
- order by `messages_count DESC`, then `src_chain_id ASC`, then `dst_chain_id ASC`

Projection eligibility for source rows (`message_countable_condition`):

- `crosschain_messages.stats_processed = 0`
- `crosschain_messages.dst_chain_id IS NOT NULL`
- **either** `status = completed` (any bridge) or `status = failed` on an AMB
  bridge, **or** the destination chain is not indexed by this message's bridge
  (`IndexedChains::may_observe(bridge_id, dst_chain_id) == false`)

Projection effect:

- increment directional counts for `(bridge_id, src_chain_id, dst_chain_id)`
- increment daily directional counts keyed by
  `(date, bridge_id, src_chain_id, dst_chain_id)`
- increment `crosschain_messages.stats_processed`

Read filter: `include_unindexed_chains=false` (default) excludes rows a
bridge could not have fully observed; `has_unindexed_chain` on individual
list-endpoint rows flags them either way.

### `/stats/chain/{chain_id}/messages-paths/received`

Same tables, ordering, and eligibility rule as sent paths.

Calculation:

- received: filter `dst_chain_id = chain_id`
- optionally filter source counterparties

### `/stats/chain/{chain_id}/bridged-tokens`

Source:

- aggregate counts from `stats_asset_edges`
- join display fields from `stats_assets`
- fetch token variants from `stats_asset_tokens LEFT JOIN tokens`

Returned counts:

- `input_transfers_count`
  - sum of edge `transfers_count` where `dst_chain_id = selected chain`
- `output_transfers_count`
  - sum of edge `transfers_count` where `src_chain_id = selected chain`
- `total_transfers_count`
  - `input + output`

Projection eligibility (`transfer_identity_ready_condition` for identity;
`message_countable_condition` for counting — see "Counting and identity are
separate concerns" above):

- identity maintenance runs whenever both known token endpoints are ready
  (address known, or its chain is unindexed for this bridge) and at least one
  endpoint is known — regardless of `stats_processed`
- counting (edge accumulation + `stats_processed` increment) additionally
  requires `stats_processed = 0` and the parent message to satisfy
  `message_countable_condition`

Projection behavior:

- resolve or create a logical `stats_asset`, merging two existing components
  if the transfer bridges them (union-find; see above)
- link src/dst tokens into `stats_asset_tokens`
- increment one `stats_asset_edges` row per
  `(stats_asset_id, bridge_id, src_chain_id, dst_chain_id)`, unless a decimals
  conflict skips the edge contribution specifically (see above)
- set `stats_asset_id` on transfers (survives even a decimals-conflict skip)
- increment `crosschain_transfers.stats_processed`

Amount semantics:

- `stats_asset_edges.cumulative_amount` uses one sticky `amount_side`
- new edges prefer source side when the source chain was actually indexed
  (`src_tx_hash` present), otherwise fall back to destination side
- decimals are filled when available, but side selection is not supposed to
  depend on async enrichment races
- a merge fold rescales the loser's amount to the winner's decimals rather
  than refusing; an unresolved decimals conflict on the counting path skips
  only that transfer's contribution, not the whole batch

Metadata semantics:

- counts can be correct before token metadata is fully enriched
- names, symbols, icons, and decimals can lag because enrichment is async
- token identity in `stats_asset_tokens` is the ICTT TokenTransferrer contract
  address, not the wrapped/underlying ERC-20. Where Home and Remote deploy via
  CREATE2 at the same address, an asset can legitimately show the same
  address on two chains — that is not a split, and the split detector
  correctly does not flag it (owner-confirmed modelling, observed on
  Avalanche NUMI/WTTC in a live run)

### `/stats/chains`

Source:

- `chains LEFT JOIN stats_chains`

Returned value:

- currently exposes `unique_transfer_users_count`

Stored snapshot values in `stats_chains`:

- `unique_transfer_users_count`
- `unique_message_users_count`

Recompute logic:

- messages:
  - distinct `(src_chain_id, sender_address)`
  - union distinct `(dst_chain_id, recipient_address)`
- transfers:
  - distinct `(token_src_chain_id, sender_address)`
  - union distinct `(token_dst_chain_id, recipient_address)`

Then:

- group by `chain_id`
- rebuild `stats_chains`
- left join from `chains` ensures known chains without a stats row can still be
  returned as `0`

Chain visibility uses `IndexedChains::configured_union()` — the union across
every configured bridge, not a per-bridge set, since `/stats/chains` and
`GetChains` are chain directories with no bridge dimension of their own; the
same accessor backs both, so they cannot disagree.

Zero-chain visibility is service-wide and configurable:

- `stats.include_zero_chains` (env:
  `INTERCHAIN_INDEXER__STATS__INCLUDE_ZERO_CHAINS`), default `true`
- when `true`: `/stats/chains` and message-path endpoints include known chains
  from `chains` even when the aggregated stats row is missing or zero
  - `/stats/chains` keeps its `chains LEFT JOIN stats_chains` shape
  - message-path endpoints drive the query from `chains` (excluding the
    selected chain) and left-join `stats_messages` / aggregated
    `stats_messages_days`
  - with explicit `counterparty_chain_ids`, message-path endpoints still drive
    from `chains`, restrict rows to the requested counterparties, exclude the
    selected chain itself, and return zero-valued rows for requested
    counterparties that exist in `chains` but have no aggregate row
- when `false`: both families return only rows with positive aggregated stats
  - `/stats/chains` filters
    `COALESCE(sc.unique_transfer_users_count, 0) > 0 OR COALESCE(sc.unique_message_users_count, 0) > 0)`
    inside the ranked SQL, preserving keyset pagination
  - message-path endpoints keep their current stats-table-driven behavior

## Refresh and Recalculation Model

### Live request-time queries

Endpoints:

- `/stats/common`
- `/stats/daily`

Behavior:

- execute direct DB queries every request
- no separate recalculation schedule

### Incremental near-realtime projection

Endpoints:

- `/stats/chain/{chain_id}/bridged-tokens`
- `/stats/chain/{chain_id}/messages-paths/sent`
- `/stats/chain/{chain_id}/messages-paths/received`

Behavior:

- refreshed when a batch (final or `Partial`) is flushed by message-buffer
  maintenance — identity maintenance runs for every flushed key; counting
  additionally requires eligibility
- not immediate on raw event arrival
- depends on message finality **or** the observability-horizon exception,
  then maintenance cadence

Main knob:

- `INTERCHAIN_INDEXER__BUFFER_SETTINGS__MAINTENANCE_INTERVAL`
- default: `500ms`

Interpretation:

- lower interval can reduce lag between canonical finalization and visible
  projected stats
- lower interval also increases maintenance overhead

### Periodic full recomputation

Endpoint:

- `/stats/chains`

Behavior:

- recomputed by background worker
- first recomputation runs immediately on startup
- later recomputations run after sleeping the configured period

Main knob:

- `INTERCHAIN_INDEXER__STATS__CHAINS_RECALCULATION_PERIOD_SECS`
  (`stats.chains_recalculation_period_secs`)
- default: `3600`
- `0` disables periodic refresh

## Backfill Semantics

### What startup backfill does

`INTERCHAIN_INDEXER__STATS__BACKFILL_ON_START=true`
(`stats.backfill_on_start = true`) triggers a startup catch-up pass that
projects historical canonical rows whose stats were not yet built, using two
sequential phases (messages, then transfers) each bounded by its own
monotonic id cursor.

It is useful when:

- stats tables are introduced after canonical data already exists
- canonical rows exist but derived stats were never projected
- a maintenance or restore procedure leaves backlog with `stats_processed = 0`
- a bridge's configured chain set widened, making previously-deferred rows
  newly countable

### What startup backfill does not do

It is not a normal steady-state refresh mode.

It should not normally remain enabled forever because:

- it adds startup work
- on large datasets it can slow startup materially
- once rows are already processed it mostly becomes wasted scanning

It is also not a full recomputation mechanism by itself:

- it only processes rows with `stats_processed = 0`
- if rows were already marked processed, turning it on again will not rebuild
  them
- it does not drain the `pending_messages` cold-tier backlog by itself — a
  cold entry only re-enters the hot buffer through a new blockchain event for
  that message key (see `gotchas.md`)

### Relationship to projection logic

Backfill reuses the same logic as live projection.

Same functions:

- `project_messages_batch(...)`
- `project_transfers_batch(...)`
- `message_countable_condition(...)` / `transfer_identity_ready_condition(...)`

Different source of candidate rows:

- live path: just-flushed batch from buffer maintenance
- backfill path: queried backlog of canonical rows with `stats_processed = 0`,
  scanned in two independently-cursored phases

## Invariants

- stats are downstream of canonical persistence
- message-path projection counts a message once it is either protocol-final
  (per the shared finality predicate) or its destination chain is no longer
  indexed by its bridge — not "protocol status `Completed`" alone
- transfer counting requires the parent message to satisfy the same rule;
  transfer *identity* maintenance runs independently of counting and may
  re-run for an already-counted transfer
- `stats_processed` prevents normal double counting and is never reset once
  set
- `stats_chains` is a snapshot table, not an append-only aggregate, and has no
  bridge dimension — visibility is a cross-bridge union, not per-bridge
- bridged-token counts can be ahead of token metadata enrichment
- message-path counts are directional; `A -> B` and `B -> A` are different rows
- `IndexedChains` is built once from in-memory config, never from
  `bridge_contracts`; the same instance is threaded through live projection,
  backfill, and the read-side filter

## Failure Modes / Observability

- projected stats can lag if flushed rows have not yet gone through buffer
  maintenance
- `/stats/chains` can lag until the next recomputation cycle
- `/stats/common` and `/stats/daily` can be slow on large canonical tables
  because they issue request-time scans / counts
- enabling startup backfill on a large database can noticeably increase startup
  time
- token metadata for bridged tokens can remain partially blank until async
  enrichment succeeds
- `STATS_INDEXED_CHAINS` (gauge, per bridge) — chains indexed per bridge as
  seen by stats eligibility, set once at startup
- `STATS_TRANSFERS_DEFERRED_TOTAL{reason}` — deferral *events* (not distinct
  rows: a row re-evaluates every time its canonical key is flushed again),
  `identity_incomplete` or `awaiting_confirmation`
- `STATS_ASSET_MERGES_TOTAL{outcome}` — `merged` or `refused_chain_collision`
- `STATS_ASSET_MERGE_REPOINTED_TRANSFERS` — histogram of transfer rows
  repointed per successful merge
- `STATS_EDGE_MIXED_AMOUNT_SIDE_TOTAL`, `STATS_EDGE_RESCALED_FOLD_TOTAL{mode}`,
  `STATS_EDGE_DECIMALS_CONFLICT_TOTAL` — edge-fold and counting-path decimals
  diagnostics (`stats/metrics.rs`)

Useful operational signals:

- startup logs for stats backfill progress and per-bridge indexed-chain counts
- startup logs for `stats_chains` recomputation success / failure
- buffer maintenance logs and metrics, because those gate projected stats
- `.memory-bank/runbooks/runtime-verification.md` — read-only SQL canaries
  and diagnostics for confirming eligibility and asset-merge behavior
  directly against a live database

## Edge Cases / Gotchas

- `/stats/common` and `/stats/daily` belong to the same API service, but unlike
  the richer stats endpoints they are not backed by derived stats tables
- `unique_message_users_count` exists in `stats_chains` but is not exposed by
  the current `/stats/chains` API
- projected stats are near-realtime, not instant: they depend on finality (or
  the observability-horizon exception) and maintenance timing
- backfill should be treated as a catch-up tool, not a permanent operational
  default
- a transfer counted while its destination chain was unindexed stays counted
  after that chain is later configured, even if the movement never completed
  — accepted inaccuracy confined to the opt-in `include_unindexed_chains`
  slice
- a bridge removed from `bridges.json` does not hide or recount its rows —
  `may_observe` stays permissive for an absent bridge by design (ADR-004
  Decision 5); only a bridge *present* with zero contracts is restrictive
- a decimals conflict on the counting path is not the same failure as a
  union-find merge refusal — see "Decimals conflict on the counting path"
  above for how to read `stats_asset_id` in each case

## Read-Time Filterability Constraints (verified 2026-07-20; extended by ADR-004)

Discovered while designing per-frontend API filtering; constrains what
filters the stats endpoints can honor without projection rework. Extended by
the `include_unindexed_chains` / `has_unindexed_chain` / `indexed_chain_ids`
surface added by ADR-004's read-side work.

Filterability matrix (as of the bridge-qualified stats rebuild,
`m20260720_120000_add_read_filters_and_bridge_stats`, plus the observability
filter):

| Endpoint | chain / counterparty | bridge | unindexed-chain filter | notes |
| --- | --- | --- | --- | --- |
| `/stats/common`, `/stats/daily` | yes (canonical WHERE) | yes (canonical WHERE) | n/a (not eligibility-gated) | counted per request from canonical tables |
| `GetMessages*`, `GetTransfers*` (canonical list endpoints) | yes | yes | **yes** (`include_unindexed_chains`, `ChainBridgeFilter.only_indexed_by_bridge`) | rows also carry `has_unindexed_chain` |
| message-paths (sent/received) | yes | yes | **yes** (`push_indexed_pairs_predicate`) | bridge filter + collapse over `stats_messages` / `stats_messages_days` |
| bridged-tokens | yes | yes | **yes** | bridge filter inside the `stats_asset_edges` aggregate, collapsed per asset |
| `/stats/chains`, `GetChains` | subject-row `chain_ids` only | no | implicit — union of every configured bridge's set (`configured_union()`) | global unique-user snapshots; no counterparty/bridge filter |
| `GetBridges` | n/a | n/a | n/a | reports each bridge's own `indexed_chain_ids`, not a filter |

- **The three additive aggregates carry `bridge_id`.** `stats_messages
  (bridge_id, src, dst)`, `stats_messages_days (date, bridge_id, src, dst)`, and
  `stats_asset_edges (asset, bridge_id, src, dst)` are bridge-qualified: message
  paths and bridged tokens accept an optional `bridge_ids` filter and collapse
  the bridge dimension (SUM) before ordering/pagination. An absent/blank filter
  reproduces the prior bridge-collapsed response. `stats_assets` /
  `stats_asset_tokens` remain global (asset identity is not bridge-specific);
  only the movement/count edges are bridge-qualified. `stats_chains
  (chain_id)` still has no bridge dimension.
- **Chain/counterparty filters are cheap exactly where aggregation happens
  at query time**: `/stats/common` and `/stats/daily` count canonical tables
  per request (WHERE-clause change); bridged-tokens aggregates
  `stats_asset_edges` per request, so a counterparty (src/dst set) and bridge
  condition is read-side only; message-paths accepts `chain_id` +
  `counterparty_chain_ids` + `bridge_ids`, composed through `AND`.
- **The unindexed-chain filter is also cheap at query time.** It renders as an
  additional `OR`/`NOT IN` disjunct built from `IndexedChains`
  (`chain_unindexed_condition` for messages/transfers projection,
  `ChainBridgeFilter.only_indexed_by_bridge` for canonical list reads,
  `push_indexed_pairs_predicate` for the raw-SQL stats endpoints) — no new
  table, no precomputed flag column.
- **Unique-user counts are non-additive.** `stats_chains` values cannot be
  re-aggregated for bridge or counterparty subsets from any exact
  pre-aggregation — the same address would fall into many cells. Exact
  filtered uniques require raw `COUNT(DISTINCT ...)` at read time;
  mergeable HyperLogLog sketches per `(chain, role, bridge, counterparty)`
  cell are the standard approximate alternative; keying `stats_chains` by
  `(chain_id, bridge_id)` is exact for single-bridge filters only. This is why
  `/stats/chains` remains bridge-unaware, and why it uses the cross-bridge
  union rather than any per-bridge restriction for chain visibility.

Candidate designs and the phased delivery decision were recorded during the
per-frontend chain-filtering follow-up scoping. The bridge dimension on
message-paths / bridged-tokens was delivered in a later iteration. The
observability-horizon eligibility rule and the `include_unindexed_chains` /
`has_unindexed_chain` / `indexed_chain_ids` surface were delivered together on
one branch (see ADR-004).

## Change Triggers

Update this note when:

- new `/stats/*` endpoints are added
- read-time filter capabilities of stats endpoints change (chain /
  counterparty / bridge / unindexed-chain filters, or a bridge dimension is
  added to projections)
- calculation rules for `stats_messages*`, `stats_asset*`, or `stats_chains`
  change
- `stats_processed` semantics change
- `IndexedChains::may_observe` semantics, or its callers, change
- asset union-find merge or decimals-conflict handling changes
- startup backfill or periodic recompute behavior changes
- `/stats/common` or `/stats/daily` are replaced by projected or externalized
  implementations
- the product boundary between embedded interchain stats and the standalone
  stats service changes

## Open Questions

- Should `/stats/common` and `/stats/daily` remain request-time canonical-table
  queries, or be replaced by projected / externalized implementations?
- Should `unique_message_users_count` be exposed through the public API?
- If projection logic changes materially, what is the canonical full
  reprojection playbook beyond the current `stats_processed = 0` catch-up path?
- Should `stats_chains` ever gain a bridge dimension via approximate sketches,
  or does the exact-uniques requirement keep it bridge-unaware indefinitely?
