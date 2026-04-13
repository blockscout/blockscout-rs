# Codebase Review

## Executive Summary

The repo already has a solid technical split between server wiring, indexing
logic, storage, migrations, and API types. For agent-oriented documentation,
the strongest existing assets are the architecture summary, gotchas, rules, and
ADRs. The main gap was coverage: a new agent could discover conventions, but
not quickly build a mental model of the repo or find the right source files for
 common questions.

From a codebase perspective, the dominant complexity sits in runtime semantics
rather than framework usage. The hard parts are message finality, unknown-chain
policy, checkpoint-safe buffering, and statistics projection. Those areas are
also the highest-value targets for durable research notes.

## Architecture Strengths

- Clear crate split between server, logic, entities, migrations, and proto
- Good separation between startup wiring and protocol-specific indexing logic
- `CrosschainIndexer` provides a clean extension seam for future bridge support
- `MessageBuffer` gives the system an explicit model for partially observed
  cross-chain state instead of forcing premature persistence
- Stats are treated as projections from canonical tables rather than direct
  side effects of log handling
- Existing `.memory-bank/` structure already supports rules, gotchas, ADRs, and
  reusable workflows

## Complexity Hotspots

- Avalanche event processing in
  `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
  - large file with multiple responsibilities: stream orchestration, event
    handling, chain filtering, blockchain ID resolution, and buffer mutation
- Message finality in
  `interchain-indexer-logic/src/indexer/avalanche/consolidation.rs`
  - correctness depends on combining multiple event families and transfer
    states
- Buffer maintenance in
  `interchain-indexer-logic/src/message_buffer/maintenance.rs`
  - correctness depends on transaction boundaries, cursor advancement, and
    compare-and-swap-style hot-tier cleanup
- Stats projection in
  `interchain-indexer-logic/src/stats/projection.rs`
  - subtle eligibility rules and incremental `stats_processed` semantics
- Config interactions between JSON files and runtime filtering in
  `interchain-indexer-server/src/config.rs` and the Avalanche indexer

## Operational Risks

- Misconfigured chains or contracts can silently reduce observed coverage by
  filtering events before they ever reach the buffer
- `process_unknown_chains` changes both indexing behavior and blockchain-ID
  persistence semantics
- Buffer maintenance is periodic, so “seen on chain” and “available in final
  tables” are intentionally decoupled
- Stats projection depends on incremental processed markers; mistakes here can
  create missed counts or double counting
- Token and chain metadata services use cache-based behavior that can make
  failures sticky for a period of time

## Testing Posture

- The repo contains integration tests and Avalanche end-to-end style tests
- Config parsing is covered by focused tests in server config code
- Runtime-critical areas have some coverage, but are still knowledge-dense and
  require source tracing to understand
- DB and network-dependent tests are intentionally opt-in, which is pragmatic
  but raises the value of accurate documentation for non-trivial flows

## Onboarding Friction

The main onboarding problem is not “how to build the service,” but “where a
specific behavior actually lives.” New contributors or agents will usually need
help locating:

- startup wiring vs per-bridge runtime logic
- where a raw Avalanche log becomes a message-buffer mutation
- where finality is decided
- when data moves from pending/hot state into canonical tables
- why an event was filtered or why a stat was not counted yet

That is why this Phase 1 documentation adds an exploration map, glossary, and
research library entrypoint.

## Recommended Research Priorities

- message lifecycle from raw logs to finalized rows
- Avalanche bridge filtering and `home_chain_id` / `process_unknown_chains`
- blockchain ID resolution and persistence rules
- message buffer persistence and checkpoint advancement
- config loading and validation
