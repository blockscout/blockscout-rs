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
  - reusable catchup/real-time log streaming primitive that indexers can build on

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

## If You Need to Understand Incoming ICTT Reconstruction / ICM Payload Decoding

- `interchain-indexer-logic/src/indexer/avalanche/ictt_payload.rs`
  - decodes `TeleporterMessage.message` into a `TransferrerMessage`,
    classifies the hop (`REGISTER_REMOTE` / `SINGLE_HOP_SEND` /
    `SINGLE_HOP_CALL` / `MULTI_HOP_SEND` / `MULTI_HOP_CALL`), enforces the
    canonicity round-trip that rejects trailing bytes
- `interchain-indexer-logic/src/indexer/avalanche/consolidation.rs`
  - `classify_payload` / `ictt_completeness` (finality classification),
    `try_reconstruct_transfer` / `build_reconstructed_transfer` (the
    reconstruction builder), `build_transfer` (the ordinary `send`-driven
    builder)
- `interchain-indexer-logic/src/indexer/avalanche/metrics.rs`
  - per-outcome reconstruction metric
- `interchain-indexer-logic/src/indexer/avalanche/abi.rs`
  - `TransferrerMessage` and per-hop-type ABI structs
- `interchain-indexer-server/src/config.rs`
  - `reconstruct_incoming_ictt_transfers` per-bridge kill switch (default `true`)
- then continue to:
  - `.memory-bank/gotchas.md` — "Message Finality is Complex"
  - `.memory-bank/research/message-lifecycle.md` — Layer 2 §8
  - `.memory-bank/research/avalanche-bridge-filtering.md` — point 2 in "Post-filter"

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

- `.memory-bank/research/message-lifecycle.md`
  - **start here** — end-to-end lifecycle research covering the generic
    pipeline (LogStream, buffer, maintenance, checkpoints, persistence) and
    Avalanche as the reference realization. Two-layer structure: Layer 1 is
    reusable for any future indexer; Layer 2 is Avalanche-specific. Future
    indexers get separate research notes referencing the generic layer.
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

- `interchain-indexer-logic/src/stats/indexed_chains.rs`
  - **start here for eligibility** — `IndexedChains`, `may_observe`, and the
    shared SQL condition builders (`message_countable_condition`,
    `transfer_identity_ready_condition`, `chain_unindexed_condition`) used by
    both live projection and backfill
- `interchain-indexer-logic/src/stats/projection.rs`
  - projection of canonical rows into stats tables; asset union-find merge
    (`merge_assets`, `ensure_asset_for_transfer`); decimals-conflict handling
- `interchain-indexer-logic/src/stats/metrics.rs`
  - eligibility, merge, and decimals-conflict metrics
- `interchain-indexer-logic/src/stats/service.rs`
  - backfill and recomputation orchestration; `apply_stats_for_flushed_batch`
    (the live projection hook, runs for every flushed entry, not only final)
- `interchain-indexer-logic/src/filters.rs`
  - `ChainBridgeFilter` — read-side SeaORM condition builder consuming
    `IndexedChains` for the unindexed-chain opt-in filter
- `interchain-indexer-server/src/services/bridge_proto.rs`
  - builds the `Bridge` proto message, including `indexed_chain_ids`
- `interchain-indexer-server/src/server.rs`
  - startup backfill, `IndexedChains::from_bridges` construction, and
    periodic stats chains worker
- `.memory-bank/adr/004-stats-observability-horizon-and-asset-union-find.md`
  - design rationale for the eligibility rule and asset merge
- `.memory-bank/research/stats-projection.md`
  - durable walkthrough for stats projection semantics
- `.memory-bank/research/stats-subsystem.md`
  - full stats API surface, eligibility rule, asset merge, and read-filter
    surface

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
