# Database Subsystem: Schema and DB Interaction Layer

## Scope

This note covers the current database subsystem as a structural slice of the
service:

- how tables are grouped by role
- which parts of the source config are mirrored into the database and why
- where direct database interaction code lives in the Rust codebase
- how generated SeaORM entities relate to migrations and manual extensions

This note intentionally does **not** go deep on endpoint enrichment behavior in
`ChainInfoService` / `TokenInfoService`, or on stats API behavior in
`StatsService`. Those services are mentioned only as DB-backed consumers.

## Short Answer

The service uses a hybrid database layer.

The main public DB abstraction is `InterchainDatabase`, but it is not the only
place that performs direct DB work. The message-buffer write path and the stats
projection/query path also contain direct SeaORM / SQL code in specialized
modules.

The schema naturally splits into these table families:

- config/reference tables
- view tables
- staging tables
- service / operational tables
- metadata / cache tables
- stats / projection tables

JSON config is still needed because it contains startup-time operational config
that is not fully represented in DB tables. The DB copy exists because runtime
code needs relational metadata, foreign-key targets, joinable reference data,
and runtime-discovered rows that do not originate from static JSON.

## Why This Matters

Future work on schema changes, indexing flow changes, or DB-layer refactors
needs a stable answer to three questions:

- which tables represent canonical user-visible data versus temporary or service
  state
- which code paths are allowed to touch those tables directly
- which parts of the repo are the database layer versus DB-backed consumers

Without that separation, it becomes difficult to reason about ownership,
transaction boundaries, and whether a new feature belongs in the DB facade, a
specialized persistence module, or a higher-level service.

## Source-of-Truth Files

- `interchain-indexer-migration/src/migrations_up/m20251030_000001_initial_up.sql`
- `interchain-indexer-migration/src/migrations_up/m20260312_175120_add_stats_tables_up.sql`
- `interchain-indexer-logic/src/database.rs`
- `interchain-indexer-logic/src/bulk.rs`
- `interchain-indexer-logic/src/message_buffer/persistence.rs`
- `interchain-indexer-logic/src/stats/projection.rs`
- `interchain-indexer-logic/src/stats_chains_query.rs`
- `interchain-indexer-logic/src/bridged_tokens_query.rs`
- `interchain-indexer-server/src/server.rs`
- `interchain-indexer-server/src/config.rs`
- `interchain-indexer-entity/src/lib.rs`
- `interchain-indexer-entity/src/manual/mod.rs`
- `scripts/bigdecimal_rename.sh`

## Key Types / Tables / Contracts

### Main DB Abstractions

- `InterchainDatabase`
  - main public DB facade
- `batched_upsert()` / `run_in_batches()`
  - low-level DB batching helpers
- message-buffer persistence helpers
  - direct cold-tier / flush / checkpoint writes
- stats projection helpers
  - direct writes from canonical rows into stats tables
- stats query helpers
  - raw SQL read models for stats endpoints

### Table Families

#### Config / Reference Tables

- `chains`
- `bridges`
- `bridge_contracts`

These are seeded from startup config and then used as relational reference
tables across the rest of the schema.

#### View Tables

- `crosschain_messages`
- `crosschain_transfers`

These are persisted canonical tables whose contents are directly exposed by the
service APIs.

#### Staging Tables

- `pending_messages`

This table stores temporary cold-tier state for partially observed messages.

#### Service / Operational Tables

- `indexer_checkpoints`
- `indexer_failures`

These tables hold shared indexer/runtime state rather than user-facing canonical
message data.

#### Metadata / Cache Tables

- `tokens`
- `avalanche_icm_blockchain_ids`

These tables hold runtime metadata discovered or refreshed outside the static
config files.

#### Stats / Projection Tables

- `stats_assets`
- `stats_asset_tokens`
- `stats_asset_edges`
- `stats_chains`
- `stats_messages`
- `stats_messages_days`
- stats marker columns on canonical tables:
  - `crosschain_messages.stats_processed`
  - `crosschain_transfers.stats_processed`
  - `crosschain_transfers.stats_asset_id`

These tables and marker columns support derived statistical views rather than
primary indexing state.

## Step-by-Step Flow

### 1. Schema Definition

The schema is defined by SQL migrations, not by the generated entities.

- the initial migration defines the base reference, view, staging, service, and
  metadata tables
- the later stats migration adds stats tables and stats marker columns

Generated entities reflect the migrated schema, but are downstream artifacts.

### 2. Startup Config Seeding

At startup, the server loads `chains.json` and `bridges.json`, converts their
models into SeaORM active models, and upserts:

- `chains`
- `bridges`
- `bridge_contracts`

This step seeds the relational reference tables from static config, but it does
not replace the config files themselves. Some operational config remains
JSON-only.

### 3. Canonical Indexing Write Path

Protocol-specific indexers do not write canonical rows directly on every event.
Instead, they mutate the message buffer. During maintenance:

1. stale hot entries are offloaded into `pending_messages`
2. consolidatable entries are flushed into:
   - `crosschain_messages`
   - `crosschain_transfers`
3. finalized entries are removed from `pending_messages`
4. `indexer_checkpoints` is updated conservatively from buffer cursors
5. finalized canonical rows are projected into stats tables in the same
   maintenance transaction

This means the authoritative canonical write path is split between the buffer
maintenance algorithm and specialized persistence/projection modules.

### 4. Runtime Metadata Writes

Not all DB rows originate from startup config.

- Avalanche blockchain-ID resolution can ensure `chains` rows exist and upsert
  `avalanche_icm_blockchain_ids`
- token metadata fetching can upsert `tokens`
- token metadata propagation can enrich stats tables based on existing
  `stats_asset_tokens` / `stats_asset_edges` links

These are runtime-discovered or runtime-refreshed metadata flows.

### 5. Read Path

Read behavior splits into two major categories.

#### Canonical API Reads

`InterchainDatabase` serves the primary message/transfer read surfaces over:

- `crosschain_messages`
- `crosschain_transfers`

Higher-level API services then enrich those rows with:

- chain metadata via `ChainInfoService`
- token metadata via `TokenInfoService`

Those enrichment services are auxiliary internal services, not the core DB
layer.

#### Stats Reads

Stats reads are served from stats tables through specialized query modules and a
small orchestration service (`StatsService`) that sits above them.

## Invariants

- Migrations are the schema source of truth.
- `interchain-indexer-entity/src/codegen/` is generated code and may be
  overwritten.
- manual entity helpers must live in `interchain-indexer-entity/src/manual/`
  rather than in generated files
- config/reference rows are seeded via upsert, not by rebuilding the whole DB
  from JSON on every startup
- view tables are persisted canonical tables, not SQL `VIEW`s
- direct DB interaction is concentrated in a hybrid layer:
  - a general facade (`InterchainDatabase`)
  - specialized direct DB modules for buffer persistence and stats logic

## Failure Modes / Observability

- config parsing can fail before seeding because startup config models use
  strict deserialization
- stale config/reference rows can remain in the DB because startup seeding is
  upsert-oriented rather than full reconciliation
- large writes must respect PostgreSQL bind limits; batching helpers exist for
  that reason
- manual edits inside generated entity files are lost on regeneration

Operationally, DB-layer activity is easiest to inspect through:

- startup logs around config loading and DB seeding
- buffer maintenance logs and metrics
- stats backfill / stats recomputation logs
- canonical and stats table contents in PostgreSQL

## Edge Cases / Gotchas

- JSON-to-DB conversion is intentionally lossy
  - example: RPC provider config is not stored in `chains`
  - example: `indexer_type`, `process_unknown_chains`, and `home_chain_id` are
    not represented by the `bridges` table alone
- some DB metadata is created at runtime and therefore cannot come only from
  source config
  - `avalanche_icm_blockchain_ids`
  - dynamically ensured `chains` rows
  - token metadata in `tokens`
- the database layer is not a single repository file or package today; it is a
  hybrid split across facade and specialized modules

## Change Triggers

Update this note when:

- a migration adds, removes, or materially repurposes tables
- a new indexer begins using shared service tables differently
- startup config seeding changes from upsert-style mirroring to strict
  reconciliation
- the hybrid DB layer is refactored into a clearer `db/` module tree or a
  different abstraction shape
- stats projection ownership moves entirely into or out of `database.rs`
- entity generation workflow changes

## Open Questions

- Should the current hybrid database layer become an explicit `db/` subtree with
  clearer internal ownership boundaries?
- Should the public DB facade remain broad, or be decomposed into several
  narrower store/query abstractions while keeping one DB package boundary?
- If config seeding ever becomes strict reconciliation instead of additive
  upsert, what protections should exist for service and canonical tables that
  depend on seeded reference rows?
