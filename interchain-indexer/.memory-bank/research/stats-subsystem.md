# Interchain Stats Subsystem

## Scope

This note covers the embedded stats subsystem inside `interchain-indexer`:

- supported stats API endpoints
- the underlying stats tables and query paths
- how values are calculated
- which endpoints read live canonical data vs precomputed stats
- refresh and backfill behavior

This note does not cover the standalone external stats service in detail. Where
the subsystem boundary depends on product context outside this repo, that is
called out explicitly as user-provided context rather than code-derived fact.

## Short Answer

`interchain-indexer` contains an embedded stats subsystem because some
interchain analytics need domain-specific read models that are not well
represented as simple counters or line-chart series. In this repo, those read
models are materialized into `stats_*` tables and exposed via dedicated stats
endpoints.

The subsystem is not uniform. It has three refresh modes:

- direct request-time queries over canonical tables
- incremental projection into `stats_*` tables during finalized flushes
- periodic full recomputation for per-chain user counters

Backfill does not use separate calculation logic. It reuses the same projection
functions as the live incremental path and only catches up rows whose
`stats_processed = 0`.

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
- some refresh on every finalized flush
- one refreshes on a background period

Operationally, this affects:

- query cost
- freshness expectations
- whether a value can lag after indexing
- whether startup backfill is needed
- how to recover after schema or projection changes

## Source-of-Truth Files

- `interchain-indexer-proto/proto/v1/stats.proto`
- `interchain-indexer-proto/proto/v1/api_config_http.yaml`
- `interchain-indexer-server/src/services/stats.rs`
- `interchain-indexer-server/src/server.rs`
- `interchain-indexer-server/src/settings.rs`
- `interchain-indexer-logic/src/stats/service.rs`
- `interchain-indexer-logic/src/stats/projection.rs`
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

### Core service / orchestration types

- `StatsService`
- `BridgedTokenListRow`
- `StatsChainListRow`

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

## Data Sources

The embedded stats subsystem uses four effective data-source patterns.

### 1. Canonical interchain tables

Used directly by request-time queries:

- `crosschain_messages`
- `crosschain_transfers`

### 2. Projected message stats tables

Materialized from finalized canonical messages:

- `stats_messages`
- `stats_messages_days`

### 3. Projected asset/token stats tables

Materialized from finalized canonical transfers:

- `stats_assets`
- `stats_asset_tokens`
- `stats_asset_edges`

### 4. Periodic snapshot table

Rebuilt from canonical tables:

- `stats_chains`

## Endpoint Matrix

| Endpoint | Data source | Freshness model | Refresh trigger | Configurable period |
| --- | --- | --- | --- | --- |
| `/api/v1/stats/common` | Direct query over `crosschain_messages` + `crosschain_transfers` | Request-time live DB read | Every request | No |
| `/api/v1/stats/daily` | Direct query over `crosschain_messages` + `crosschain_transfers` | Request-time live DB read | Every request | No |
| `/api/v1/stats/chain/{chain_id}/bridged-tokens` | `stats_asset_edges` + `stats_assets` + `stats_asset_tokens` + `tokens` | Pre-calculated, near-realtime | Projection during finalized batch flush | Indirectly via buffer maintenance interval |
| `/api/v1/stats/chain/{chain_id}/messages-paths/sent` | `stats_messages` or `stats_messages_days` | Pre-calculated, near-realtime | Projection during finalized batch flush | Indirectly via buffer maintenance interval |
| `/api/v1/stats/chain/{chain_id}/messages-paths/received` | `stats_messages` or `stats_messages_days` | Pre-calculated, near-realtime | Projection during finalized batch flush | Indirectly via buffer maintenance interval |
| `/api/v1/stats/chains` | `chains LEFT JOIN stats_chains` | Pre-calculated periodic snapshot | Background full recomputation worker | Yes |

## Step-by-Step Flow

### 1. Canonical rows are persisted first

The indexer and message buffer persist canonical rows into:

- `crosschain_messages`
- `crosschain_transfers`

Stats are downstream of canonical persistence. They are not direct side effects
of raw event handling.

### 2. Finalized flush can project stats inline

When message-buffer maintenance flushes finalized entries, it calls
`StatsService::apply_stats_for_finalized_batch(...)` inside the same DB
transaction.

That method:

- collects finalized message primary keys
- calls `project_messages_batch(...)`
- finds associated unprocessed transfers
- calls `project_transfers_batch(...)`

This is the main near-realtime stats path.

For detailed message-projection semantics, see `stats-projection.md`.

### 3. Startup backfill reuses the same projection rules

When `stats_backfill_on_start = true`, startup calls
`backfill_stats_until_idle_with_token_enrichment()`.

That method repeatedly runs `backfill_stats_projection_round(...)` until no
eligible rows remain. The round function selects canonical rows with
`stats_processed = 0` and passes them into the same projection functions used by
the live finalized-flush path.

Backfill is therefore a catch-up wrapper around projection, not separate stats
logic.

For the detailed relationship between startup backfill and projection
eligibility rules, see `stats-projection.md`.

### 4. `stats_chains` is refreshed separately

`stats_chains` does not use the incremental finalized-batch projection path.
Instead, a background worker periodically runs `recompute_stats_chains()`, which
rebuilds the table from canonical messages and transfers.

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

Projection eligibility for source rows:

- `crosschain_messages.stats_processed = 0`
- `crosschain_messages.status = completed`
- `crosschain_messages.dst_chain_id IS NOT NULL`

Projection effect:

- increment directional counts for `(src_chain_id, dst_chain_id)`
- increment daily directional counts keyed by
  `(init_timestamp.date(), src_chain_id, dst_chain_id)`
- increment `crosschain_messages.stats_processed`

### `/stats/chain/{chain_id}/messages-paths/received`

Same tables and ordering as sent paths.

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

Projection eligibility for source rows:

- `crosschain_transfers.stats_processed = 0`
- parent message status is `completed`

Projection behavior:

- resolve or create a logical `stats_asset`
- link src/dst tokens into `stats_asset_tokens`
- increment one `stats_asset_edges` row per
  `(stats_asset_id, src_chain_id, dst_chain_id)`
- set `stats_asset_id` on transfers
- increment `crosschain_transfers.stats_processed`

Amount semantics:

- `stats_asset_edges.cumulative_amount` uses one sticky `amount_side`
- new edges prefer source side when the source chain was actually indexed
  (`src_tx_hash` present), otherwise fall back to destination side
- decimals are filled when available, but side selection is not supposed to
  depend on async enrichment races

Metadata semantics:

- counts can be correct before token metadata is fully enriched
- names, symbols, icons, and decimals can lag because enrichment is async

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

- refreshed when finalized canonical data is flushed by message-buffer
  maintenance
- not immediate on raw event arrival
- depends on message finality first, then maintenance cadence

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

- `INTERCHAIN_INDEXER__STATS_CHAINS_RECALCULATION_PERIOD_SECS`
- default: `3600`
- `0` disables periodic refresh

## Backfill Semantics

### What startup backfill does

`INTERCHAIN_INDEXER__STATS_BACKFILL_ON_START=true` triggers a startup catch-up
pass that projects historical canonical rows whose stats were not yet built.

It is useful when:

- stats tables are introduced after canonical data already exists
- canonical rows exist but derived stats were never projected
- a maintenance or restore procedure leaves backlog with `stats_processed = 0`

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

### Relationship to projection logic

Backfill reuses the same logic as live projection.

Same functions:

- `project_messages_batch(...)`
- `project_transfers_batch(...)`

Different source of candidate rows:

- live path: just-finalized batch from buffer maintenance
- backfill path: queried backlog of canonical rows with `stats_processed = 0`

## Invariants

- stats are downstream of canonical persistence
- message-path projection only counts completed messages with non-null
  destination chain
- transfer projection only counts transfers whose parent message is completed
- `stats_processed` prevents normal double counting
- `stats_chains` is a snapshot table, not an append-only aggregate
- bridged-token counts can be ahead of token metadata enrichment
- message-path counts are directional; `A -> B` and `B -> A` are different rows

## Failure Modes / Observability

- projected stats can lag if finalized rows have not yet gone through buffer
  maintenance
- `/stats/chains` can lag until the next recomputation cycle
- `/stats/common` and `/stats/daily` can be slow on large canonical tables
  because they issue request-time scans / counts
- enabling startup backfill on a large database can noticeably increase startup
  time
- token metadata for bridged tokens can remain partially blank until async
  enrichment succeeds

Useful operational signals:

- startup logs for stats backfill progress
- startup logs for `stats_chains` recomputation success / failure
- buffer maintenance logs and metrics, because those gate projected stats

## Edge Cases / Gotchas

- `/stats/common` and `/stats/daily` belong to the same API service, but unlike
  the richer stats endpoints they are not backed by derived stats tables
- `unique_message_users_count` exists in `stats_chains` but is not exposed by
  the current `/stats/chains` API
- projected stats are near-realtime, not instant: they depend on finality and
  maintenance timing
- backfill should be treated as a catch-up tool, not a permanent operational
  default

## Change Triggers

Update this note when:

- new `/stats/*` endpoints are added
- calculation rules for `stats_messages*`, `stats_asset*`, or `stats_chains`
  change
- `stats_processed` semantics change
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
