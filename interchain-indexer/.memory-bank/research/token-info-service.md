# TokenInfoService and Token Metadata Enrichment Flow

## Scope

This note covers the token metadata subsystem centered on `TokenInfoService`:

- where the service is constructed and injected
- every current usage site
- request-time token lookup behavior for API responses
- async metadata enrichment via RPC and Blockscout contracts-info
- persistence into `tokens`
- downstream stats enrichment that consumes token metadata

This note intentionally does **not** re-document the full stats schema or the
general DB-layer structure already covered by `db-schema-and-layer.md`.

## Short Answer

`TokenInfoService` is an eventually consistent metadata service layered on top
of canonical indexing.

Facts:

- API callers can request token metadata synchronously, but cache / DB misses do
  not block on RPC
- unknown tokens return a placeholder model immediately, while background tasks
  fetch and persist metadata later
- token icons are optional and come from a separate external source than
  `name` / `symbol` / `decimals`
- stats workflows can kick off metadata fetches, but only after canonical rows
  and stats projection have already committed

Inferred conclusion:

- token metadata is enrichment state, not part of the canonical indexing
  contract; callers must tolerate partial or stale token metadata

## Why This Matters

This subsystem is easy to misunderstand because it mixes:

- synchronous API reads
- background RPC fetches
- local memory caching
- negative caching for failures
- optional external icon lookup
- DB writes that can happen as a side effect of reads
- post-commit stats enrichment hooks

Without a durable note, future changes can easily break the service's latency
model, deduplication guarantees, or the boundary between canonical indexing and
best-effort enrichment.

## Source-of-Truth Files

- `interchain-indexer-logic/src/token_info/service.rs`
- `interchain-indexer-logic/src/token_info/settings.rs`
- `interchain-indexer-logic/src/token_info/blockscout_tokeninfo.rs`
- `interchain-indexer-logic/src/token_info/fetchers/fetcher.rs`
- `interchain-indexer-logic/src/token_info/fetchers/erc20.rs`
- `interchain-indexer-logic/src/token_info/fetchers/erc20_token_home.rs`
- `interchain-indexer-server/src/server.rs`
- `interchain-indexer-server/src/services/interchain_service.rs`
- `interchain-indexer-logic/src/stats/service.rs`
- `interchain-indexer-logic/src/message_buffer/maintenance.rs`
- `interchain-indexer-logic/src/database.rs`
- `interchain-indexer-migration/src/migrations_up/m20251030_000001_initial_up.sql`
- `interchain-indexer-migration/src/migrations_up/m20260312_175120_add_stats_tables_up.sql`
- `interchain-indexer-server/config/example.toml`

## Key Types / Tables / Contracts

- `TokenInfoService`
  - main orchestrator for token metadata lookups, caches, and background fetches
- `BlockscoutTokenInfoClient`
  - external icon lookup client with its own cache and per-key locks
- `TokenInfoFetcher`
  - pluggable on-chain metadata fetcher trait
- `OnchainTokenInfo`
  - on-chain metadata payload: `name`, `symbol`, `decimals`
- `TokenKey = (i64, Vec<u8>)`
  - cache / dedupe key by `(chain_id, token_address)`
- `tokens`
  - persisted token metadata cache keyed by `(chain_id, address)`
- `stats_asset_tokens`
  - chain-local token to logical stats asset mapping
- `stats_assets`
  - logical asset metadata that can be filled from `tokens`
- `stats_asset_edges`
  - aggregated token movement edges whose `decimals` may be filled from
    `tokens`
- `TokenInfoServiceSettings`
  - on-chain retry behavior and nested Blockscout icon settings

## Step-by-Step Flow

### 1. Startup constructs one shared service

Server startup creates `TokenInfoService` once from:

- `InterchainDatabase`
- provider pool built from configured chains
- `settings.token_info`

The same shared instance is then passed into `StatsService` and API services.

### 2. Current usage sites split into request-time and background modes

Current usage sites are:

- `interchain-indexer-server/src/services/interchain_service.rs`
  - request-time enrichment for transfer API responses
- `interchain-indexer-logic/src/message_buffer/maintenance.rs`
  - post-commit enrichment kickoff for newly finalized transfers
- `interchain-indexer-logic/src/database.rs`
  - startup backfill round hands discovered token keys to the service
- `interchain-indexer-logic/src/message_buffer/buffer.rs`
  - optional convenience constructor for embedders that want a buffer with
    shared token enrichment

This split is important: the same service supports both latency-sensitive reads
and non-blocking background enrichment.

### 3. Request-time API lookup prefers cache, then DB, then async fallback

When the API layer needs token metadata for source or destination tokens, it
calls `TokenInfoService::get_token_info(...)`.

That method:

1. validates `chain_id`
2. checks the in-memory cache
3. acquires a per-key mutex on cache miss
4. checks the cache again after lock acquisition
5. checks `error_cache` to avoid retry storms after recent failures
6. loads from the `tokens` table on DB hit
7. on DB miss, spawns one background fetch and returns a placeholder model

The placeholder model contains only:

- `chain_id`
- `address`
- empty `name`
- empty `symbol`
- empty `decimals`
- empty `token_icon`

This is a fact of the implementation, not an edge case.

### 4. Existing DB rows may still trigger icon refresh on reads

If a cached or DB-loaded token row exists but has no icon,
`fetch_icon_if_needed(...)` runs during the request-time path.

That path may:

- call Blockscout contracts-info for the icon
- update the `tokens` row
- propagate the refreshed metadata into stats tables
- update the in-memory cache

So request-time token reads are not purely read-only. They can trigger DB
write-back for missing icons.

### 5. Background fetch combines two metadata sources

When a token needs fresh enrichment, `fetch_token_info_from_chain_and_persist`
runs two operations in parallel:

- `try_fetch_token_info_onchain(...)`
- `try_fetch_token_icon(...)`

The metadata sources have different responsibilities:

- on-chain fetchers provide `name`, `symbol`, `decimals`
- Blockscout provides only `token_icon`

The on-chain fetcher chain currently tries:

1. direct ERC-20 contract calls
2. `ERC20TokenHome`, which first resolves the underlying ERC-20 token address
   and then reuses ERC-20 metadata fetch

### 6. Only successful on-chain metadata persists the token row

If on-chain fetch succeeds:

1. the service builds a full token model
2. it upserts the `tokens` row
3. updates the in-memory cache
4. propagates metadata into stats tables
5. clears any error-cache entry

If on-chain fetch fails:

- the service logs a warning
- records the failure time in `error_cache`
- does **not** persist a new `tokens` row from icon-only data

This keeps `tokens` anchored to successful on-chain metadata rather than to
best-effort external icon responses.

### 7. Stats workflows trigger enrichment only after projection commits

There are two stats-related kickoff paths:

- normal runtime maintenance after finalized entries are flushed and projected
- startup backfill when unprocessed stats batches are replayed

In both cases, token enrichment is kicked off only after the core projection
work is already committed. This preserves the repo-wide design goal that
indexing and stats projection do not wait on token RPC.

### 8. Stats-driven enrichment has a narrower fetch condition

`kickoff_token_fetch_for_stats_enrichment(...)` only schedules a background
fetch when the token row:

- does not exist
- has no `decimals`
- or has both `name` and `symbol` empty

Missing icon alone does not qualify. Icon refresh is mainly a request-time
concern for already-known tokens.

## Invariants

- canonical indexing does not depend on token metadata availability
- token metadata is keyed by `(chain_id, token_address)`
- API reads must tolerate partial token metadata
- background fetch spawns are deduplicated by `in_flight_fetches`
- request-time cache / DB work is deduplicated by per-key mutexes
- on-chain failures are negatively cached for `onchain_retry_interval`
- Blockscout icon results are cached separately from on-chain failures
- `tokens` rows are persisted from successful on-chain metadata, not from
  icon-only responses
- stats-table enrichment only fills missing values; it is not a full
  re-projection pass

## Failure Modes / Observability

- if no provider exists for a chain, unknown tokens on that chain cannot be
  enriched
- if RPC calls fail, the service will return placeholder metadata until the
  negative-cache TTL expires or the process restarts
- if Blockscout URL is unset, icons are simply unavailable and a warning is
  logged at service construction
- if Blockscout is transiently failing, icon lookups are logged as warnings and
  cached as `None` until retry TTL expires
- if DB upsert fails after successful metadata fetch, the service logs a warning
  and the in-memory cache is not refreshed from persisted state
- if stats propagation sees conflicting edge decimals, it logs and skips the
  overwrite

Primary observability anchors:

- `Failed to get token info`
- `Background token info fetch failed`
- `Failed to upsert token info in background`
- `Failed to fetch token info`
- `Failed to fetch token icon`
- `Failed to update token icon in database`
- `stats enrichment: not overwriting edge decimals (conflict)`

## Edge Cases / Gotchas

- unknown-token API lookups return a placeholder immediately instead of waiting
  for RPC
- request-time reads can perform write-back if the token already exists but
  lacks `token_icon`
- positive icon cache entries in `BlockscoutTokenInfoClient` are effectively
  sticky in-process, while cached `None` entries expire by retry TTL
- transient Blockscout failures and genuine "no icon" outcomes both collapse to
  cached `None`
- the service keeps only the last on-chain fetcher error when all fetchers fail
- stats-triggered enrichment does not chase icon-only gaps

## Change Triggers

Update this note when any of the following change:

- new `TokenInfoFetcher` implementations or a changed fetcher order
- `TokenInfoServiceSettings` or Blockscout settings semantics
- request-time behavior on cache / DB miss
- persistence rules for `tokens`
- icon refresh rules for existing token rows
- stats kickoff timing or eligibility rules
- stats propagation semantics into `stats_assets` / `stats_asset_edges`
- new production usage sites for `TokenInfoService`

## Open Questions

- whether positive icon cache entries should eventually expire, not just missing
  icon entries
- whether token enrichment should expose metrics beyond warning logs
- whether icon-only enrichment should ever be allowed to persist a minimal token
  row without successful on-chain metadata
