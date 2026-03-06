# Interchain Indexer

Rust microservice indexing cross-chain messages and token transfers. Currently supports Avalanche Teleporter (ICM) and ICTT protocols.

## Stack

- Rust 2021
- Tokio
- PostgreSQL + SeaORM
- Actix-web + Tonic
- Alloy

## Build & Test

Run `just` to see the available commands, or check the @justfile.

## Architecture

**Crates:**
- `interchain-indexer-server` — HTTP/gRPC server, config, service init
- `interchain-indexer-logic` — Core indexing, message buffer, log streaming
- `interchain-indexer-entity` — SeaORM entities (codegen/ is auto-generated)
- `interchain-indexer-migration` — Database migrations
- `interchain-indexer-proto` — Protobuf definitions

**Core abstractions:**
- `CrosschainIndexer` trait — Plugin interface for bridge indexers
- `MessageBuffer` — Tiered storage (memory + DB) for assembling messages
- `LogStream` — Bidirectional blockchain log streaming with checkpointing
- `Consolidate` trait — Determines message finality

For details see: .memory-bank/architecture.md

## Conventions

1. **Imports:** Use crate-level grouping (`imports_granularity=Crate`)
2. **Errors:** `anyhow::Result` internally, `thiserror` at API boundaries
3. **Logging:** `tracing` with field syntax: `tracing::info!(field = value, "message")`
4. **Async:** `#[async_trait]` for trait methods, `Arc<RwLock<T>>` for shared state
5. **Database:** Always use `on_conflict()` for upserts, batch large inserts
6. **Config:** `#[serde(deny_unknown_fields)]` — typos fail, not silently ignored
7. **Tests:** `#[ignore]` for DB tests, use `TestDbGuard` for isolation

For details see: .memory-bank/rules/

## Configuration

- **Files:** `config/avalanche/chains.json`, `config/avalanche/bridges.json`
- **Env vars:** `INTERCHAIN_INDEXER__<SECTION>__<KEY>`

## Key Decisions

See .memory-bank/adr/README.md for architectural decision records.

## Known Gotchas

1. **Message finality is complex** — Requires execution success AND ICTT completion
2. **Unconfigured chains filtered** — Events to/from chains not in bridge config are skipped (trace-logged)
3. **Config typos fail hard** — `deny_unknown_fields` rejects typos
4. **Entity regeneration overwrites codegen/** — Put customizations in manual/
5. **PostgreSQL bind limit** — Use batched operations for large inserts

For details see: .memory-bank/gotchas.md

## Memory Protocol

When you discover a non-obvious pattern or gotcha, update .memory-bank/gotchas.md
When making an architectural decision, add an ADR to .memory-bank/adr/
When corrected about a convention, update the relevant file in .memory-bank/rules/
When a new coding rule emerges, update the relevant file in .memory-bank/rules/ or create a new one if needed.

## Workflows

Reusable task procedures are in `.memory-bank/workflows/`. These are tool-agnostic —
tool-specific integrations (e.g., Claude Code `/skills`) are thin wrappers.

For GitHub Copilot Chat to discover and use project skills, enable this VS Code setting:

```json
"chat.useClaudeSkills": true
```

- `gh-issue-bug.md` — draft a GitHub bug report
- `gh-issue-improvement.md` — draft a GitHub enhancement proposal
- `gh-issue-publish.md` — publish a drafted issue via the `gh` CLI
