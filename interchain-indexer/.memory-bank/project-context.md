# Project Context

## Purpose

`interchain-indexer` is a standalone Rust microservice for indexing cross-chain
messages and token transfers. It extends the Blockscout ecosystem with a
service that models interchain activity directly, instead of depending on
single-chain explorers to assemble cross-network behavior afterward.

Today the implementation is centered on Avalanche native interop, especially
Teleporter / ICM messaging and ICTT token transfer flows. The design is broader
than that single protocol family: the service stores bridge metadata, tracks
checkpoints, consolidates partially observed messages, and exposes query APIs
for finalized cross-chain records.

## Current Scope

- Implemented protocol family: Avalanche Teleporter (ICM) and ICTT
- Implemented indexer: Avalanche native indexer
- Storage model: PostgreSQL tables for chains, bridges, contracts, finalized
  messages, transfers, pending messages, checkpoints, failures, and aggregated
  stats
- Serving layer: HTTP and optional gRPC endpoints for interchain data, status,
  and stats
- Configuration model: JSON files for chains and bridges, plus environment
  variables for runtime settings

The architecture anticipates more bridge/indexer implementations, but the
current production logic is concentrated in the Avalanche path.

## Crate Map

- `interchain-indexer-server`
  - service startup, config loading, provider construction, API services, and
    indexer spawning
  - primary entrypoints:
    - `src/bin/interchain-indexer-server.rs`
    - `src/server.rs`
    - `src/indexers.rs`
    - `src/config.rs`
    - `src/settings.rs`
- `interchain-indexer-logic`
  - core indexing logic, message buffer, log streaming, database abstraction,
    chain/token info services, and stats projection
  - primary entrypoints:
    - `src/indexer/crosschain_indexer.rs`
    - `src/indexer/avalanche/mod.rs`
    - `src/message_buffer/`
    - `src/log_stream.rs`
    - `src/database.rs`
    - `src/stats/projection.rs`
- `interchain-indexer-entity`
  - SeaORM entities generated from the schema, plus manual extensions
- `interchain-indexer-migration`
  - migration definitions and SQL sources for schema changes
- `interchain-indexer-proto`
  - protobuf definitions and generated API bindings

## Main Runtime Components

- `Settings`
  - runtime configuration assembled from env vars and defaults
  - source: `interchain-indexer-server/src/settings.rs`
- JSON config loaders
  - `chains.json` and `bridges.json` are read at startup
  - source: `interchain-indexer-server/src/config.rs`
- Provider pools
  - Alloy HTTP providers created from chain RPC definitions
  - source: `interchain-indexer-server/src/config.rs`
- `AvalancheIndexer`
  - per-bridge indexer implementation for Teleporter / ICTT
  - source: `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
- `LogStream`
  - catchup + realtime log streaming with checkpoint-aware cursors
  - source: `interchain-indexer-logic/src/log_stream.rs`
- `MessageBuffer`
  - hot + cold message assembly layer for partially observed cross-chain state
  - source: `interchain-indexer-logic/src/message_buffer/`
- `InterchainDatabase`
  - storage abstraction wrapping SeaORM operations and upserts
  - source: `interchain-indexer-logic/src/database.rs`
- `StatsService`
  - stats backfill, projections, and periodic recomputations
  - sources:
    - `interchain-indexer-logic/src/stats/service.rs`
    - `interchain-indexer-logic/src/stats/projection.rs`
- `TokenInfoService` / `ChainInfoService`
  - metadata enrichment and cache-backed chain/token lookups
  - sources:
    - `interchain-indexer-logic/src/token_info/service.rs`
    - `interchain-indexer-logic/src/chain_info/service.rs`

## External Systems

- PostgreSQL
  - primary persistent store for indexer state and API data
- Blockchain RPC endpoints
  - configured per chain in `chains.json`; used for log streaming and token
    metadata calls
- Avalanche Data API
  - used by blockchain ID resolution when Avalanche native IDs must be mapped to
    EVM chain IDs
  - source:
    `interchain-indexer-logic/src/indexer/avalanche/blockchain_id_resolver.rs`
- Optional Blockscout token info service
  - used as one of the token metadata enrichment sources
- Blockscout service launcher
  - shared infra for server bootstrapping, DB initialization, and tracing

## Configuration Model

### Static Repo Configuration

- `config/avalanche/chains.json`
  - known chains, explorer metadata, RPC providers, and native IDs
- `config/avalanche/bridges.json`
  - bridges, contracts, enablement, and filtering settings such as
    `process_unknown_chains` and `home_chain_id`

### Runtime Configuration

- env prefix: `INTERCHAIN_INDEXER__`
- config assembly source:
  - `interchain-indexer-server/src/settings.rs`
  - `interchain-indexer-server/src/bin/check-envs.rs`

Important runtime toggles include:

- DB connection and migration settings
- API pagination settings
- token info service behavior
- buffer maintenance intervals
- Avalanche indexer batch size / pull interval / Data API settings
- stats backfill and periodic stats recomputation

## Local Development Flow

Primary task runner: `just`

Common commands from `justfile`:

- `just`
  - list available tasks
- `just start-postgres`
  - run a disposable local Postgres container
- `just migrate-up`
  - apply DB migrations
- `just generate-entities`
  - regenerate SeaORM codegen into `interchain-indexer-entity/src/codegen`
- `just run`
  - start the service against Avalanche config with migrations enabled
- `just run-dev`
  - same as `run`, but loads env vars from `.env`
- `just check`
  - `cargo check` + strict clippy
- `just format`
  - `cargo sort` when available + `cargo fmt`

## Testing Flow

- `just test`
  - runs test suites including ignored tests
  - also runs ignored DB-backed tests, so it may fail unless Postgres is available; prefer `just test-with-db` for a self-contained full run
- `just test-with-db`
  - starts a temporary Postgres instance and runs tests against it
- server integration tests
  - located in `interchain-indexer-server/tests/`
- DB-backed tests
  - commonly use `TestDbGuard` or helper DB initialization utilities
- network-dependent tests
  - typically `#[ignore]` and intentionally opt-in

The repo has meaningful tests around config parsing and Avalanche flows, but
many behaviors remain easiest to understand through source tracing and research
notes.

## Current Constraints

- Current implementation depth is heavily Avalanche-specific
- Message finality is multi-step and depends on both execution and transfer
  completion semantics
- Unknown-chain handling is configurable and can change both persistence and
  filtering behavior
- Message assembly is intentionally asynchronous and tiered, so observed events
  do not map 1:1 to immediate finalized DB writes
- Stats are projection-based and near-realtime, not event-by-event realtime
