# Exploration Map

## If You Need to Understand the Whole System

- `.memory-bank/project-context.md`
  - high-level repo scope, crate responsibilities, runtime components, and local workflow
- `.memory-bank/architecture.md`
  - high-level data flow summary and core abstractions
- `interchain-indexer-server/src/bin/interchain-indexer-server.rs`
  - process entrypoint and config bootstrap
- `interchain-indexer-server/src/server.rs`
  - startup wiring, DB initialization, config loading, services, and server launch
- `interchain-indexer-server/src/indexers.rs`
  - bridge-to-indexer wiring between config and logic
- then continue to:
  - `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
  - `interchain-indexer-logic/src/message_buffer/maintenance.rs`
  - `interchain-indexer-logic/src/stats/projection.rs`

## If You Need to Understand Common Indexer Architecture

- `interchain-indexer-logic/src/indexer/crosschain_indexer.rs`
  - shared indexer trait, lifecycle, state model, and status contract
- `interchain-indexer-server/src/indexers.rs`
  - server-side mapping from bridge config to concrete indexer instances
- `interchain-indexer-logic/src/message_buffer/mod.rs`
  - shared buffering boundary used by indexers to hand off partially assembled state
- `interchain-indexer-logic/src/message_buffer/types.rs`
  - `Consolidate` and `ConsolidatedMessage` contracts that define how protocol-specific state becomes canonical storage input
- `interchain-indexer-logic/src/message_buffer/maintenance.rs`
  - shared maintenance flow that offloads pending state, flushes finalized entries, and advances checkpoints
- `interchain-indexer-logic/src/log_stream.rs`
  - reusable catchup/realtime log streaming primitive that indexers can build on

## If You Need to Understand Avalanche Indexing

- `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
  - main indexer implementation, stream orchestration, event handlers
- `interchain-indexer-logic/src/indexer/avalanche/consolidation.rs`
  - finality and message assembly rules
- `interchain-indexer-logic/src/indexer/avalanche/types.rs`
  - message/event domain types
- `interchain-indexer-server/src/indexers.rs`
  - how Avalanche indexers are instantiated per bridge
- then continue to:
  - `interchain-indexer-logic/src/indexer/avalanche/blockchain_id_resolver.rs`
  - `interchain-indexer-logic/src/message_buffer/maintenance.rs`

## If You Need to Understand Bridge Filtering

- `interchain-indexer-server/src/config.rs`
  - `BridgeConfig`, `process_unknown_chains`, `home_chain_id`
- `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
  - chain filtering logic inside event handling
- `.memory-bank/gotchas.md`
  - existing summary of configured/unknown chain behavior
- then continue to:
  - `config/avalanche/bridges.json`

## If You Need to Understand Avalanche Blockchain ID Resolution

- `interchain-indexer-logic/src/indexer/avalanche/blockchain_id_resolver.rs`
  - native Avalanche blockchain ID to EVM chain ID resolution
- `interchain-indexer-logic/src/avalanche_data_api.rs`
  - external API client
- `interchain-indexer-logic/src/database.rs`
  - persistence APIs for `avalanche_icm_blockchain_ids`
- then continue to:
  - `.memory-bank/gotchas.md`

## If You Need to Understand Message Lifecycle

- `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
  - raw logs to typed event handling and buffer mutation
- `interchain-indexer-logic/src/indexer/avalanche/consolidation.rs`
  - partial message to finalized message logic
- `interchain-indexer-logic/src/message_buffer/maintenance.rs`
  - periodic consolidation / offload / flush cycle
- `interchain-indexer-logic/src/message_buffer/persistence.rs`
  - writes into final and pending tables
- then continue to:
  - `.memory-bank/research/stats-projection.md`

## If You Need to Understand Buffer Persistence

- `interchain-indexer-logic/src/message_buffer/buffer.rs`
  - buffer structure and maintenance loop startup
- `interchain-indexer-logic/src/message_buffer/maintenance.rs`
  - maintenance planning and commit behavior
- `interchain-indexer-logic/src/message_buffer/persistence.rs`
  - DB persistence for pending and finalized items
- `interchain-indexer-logic/src/message_buffer/cursor.rs`
  - cursor merging and checkpoint inputs
- then continue to:
  - `interchain-indexer-logic/src/database.rs`

## If You Need to Understand Stats

- `interchain-indexer-logic/src/stats/projection.rs`
  - projection of canonical rows into stats tables
- `interchain-indexer-logic/src/stats/service.rs`
  - backfill and recomputation orchestration
- `interchain-indexer-server/src/server.rs`
  - startup backfill and periodic stats chains worker
- `.memory-bank/research/stats-projection.md`
  - durable walkthrough for stats projection semantics

## If You Need to Understand Service-Wide Metadata Services

- `interchain-indexer-logic/src/chain_info/service.rs`
  - `ChainInfoService` resolves and caches chain metadata used across API and stats flows
- `interchain-indexer-logic/src/chain_info/settings.rs`
  - configuration for chain-info cooldown and lookup behavior
- `interchain-indexer-logic/src/token_info/service.rs`
  - `TokenInfoService` resolves, caches, and asynchronously enriches token metadata across ingestion and stats flows
- `interchain-indexer-logic/src/token_info/settings.rs`
  - configuration for retry intervals and external token info sources
- `interchain-indexer-logic/src/token_info/blockscout_tokeninfo.rs`
  - Blockscout token info client used as one metadata source

## If You Need to Understand API Serving

- `interchain-indexer-server/src/server.rs`
  - HTTP/gRPC router registration
- `interchain-indexer-server/src/services/interchain_service.rs`
  - interchain message/transfer queries
- `interchain-indexer-server/src/services/stats.rs`
  - statistics endpoints
- `interchain-indexer-server/src/services/status.rs`
  - indexer status reporting
- `interchain-indexer-proto/proto/v1/interchain_indexer.proto`
  - core API contract definitions
- `interchain-indexer-proto/proto/v1/stats.proto`
  - stats API contract definitions
- then continue to:
  - `interchain-indexer-logic/src/database.rs`

## If You Need to Understand Config Loading

- `interchain-indexer-server/src/settings.rs`
  - env-driven settings
- `interchain-indexer-server/src/config.rs`
  - JSON config models and loaders
- `interchain-indexer-server/config/example.toml`
  - example config shape
- `justfile`
  - local run defaults and operational commands
- then continue to:
  - `config/avalanche/chains.json`
  - `config/avalanche/bridges.json`

## If You Need to Understand Database Schema and Migrations

- `interchain-indexer-migration/src/m20251030_000001_initial.rs`
  - initial migration entry
- `interchain-indexer-migration/src/migrations_up/m20251030_000001_initial_up.sql`
  - base schema SQL
- `interchain-indexer-migration/src/m20260312_175120_add_stats_tables.rs`
  - stats migration entry
- `interchain-indexer-entity/src/codegen/`
  - generated entity view of the current schema
- `interchain-indexer-entity/src/manual/mod.rs`
  - place for manual entity customizations that survive regeneration
