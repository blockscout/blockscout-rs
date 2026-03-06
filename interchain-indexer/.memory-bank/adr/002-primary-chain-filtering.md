# ADR-002: Per-Bridge Chain Filtering with `process_unknown_chains` + `home_chain_id`

**Date:** 2026-03-05

## Context

Unknown-chain filtering needs to be bridge-scoped and explicitly configurable.

Requirements:

- strict mode (only configured chains),
- allow one-configured/one-unknown messages,
- optionally narrow processing to messages involving a designated chain.

## Decision

Define bridge-level filtering in `bridges.json` with two independent fields:

```rust
process_unknown_chains: bool
home_chain_id: Option<ChainId>
```

Behavior per bridge (`h = home_chain_id`):

- `process_unknown_chains = false`, `home_chain_id = None`:
  - process only configured↔configured messages.
- `process_unknown_chains = false`, `home_chain_id = Some(h)`:
  - process configured↔configured messages only when `source == h || destination == h`.
- `process_unknown_chains = true`, `home_chain_id = None`:
  - process configured↔configured and one-configured/one-unknown messages.
- `process_unknown_chains = true`, `home_chain_id = Some(h)`:
  - process messages only when `source == h || destination == h` (unknown endpoints allowed).
- both unknown endpoints are always skipped.

Filter order in implementation:

1. Chain-config filter (`process_unknown_chains`)
2. Home-chain filter (`home_chain_id`)

Validation:

- Startup fails if `home_chain_id` is set but not present in the bridge's configured contracts.
- `ChainId (u64)` is converted to `i64` internally with range checks.

## Consequences

### Positive

- **Per-bridge control** aligns with operational intent.
- **Explicit unknown-chain policy** via `process_unknown_chains`.
- **Composability**: `home_chain_id` can narrow both configured and unknown-inclusive traffic.
- **Fail-fast validation** prevents silent misconfiguration.

### Negative

- Additional config field to maintain (`process_unknown_chains`).
- Existing configs/tests must set `process_unknown_chains` explicitly if they rely on unknown-chain handling.

### Neutral

- No DB schema changes.
- Filtering still happens before buffering/consolidation.

## Alternatives Considered

1. Keep only `home_chain_id` as unknown-chain control.
  - Rejected: cannot express "allow unknown chains without home-chain narrowing."
2. Keep global settings and pass overrides per bridge.
  - Rejected: duplicate configuration paths.
3. `home_chain_ids: Vec<ChainId>`.
  - Rejected for now: unnecessary complexity for current use cases.
4. Pair whitelist `(chain_a, chain_b)`.
  - Rejected: high config complexity and poor maintainability.

## References

- Plan: `tmp/plans/re-introduce-process-unknown-chains.md`
- Implementation: `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
- Bridge config: `interchain-indexer-server/src/config.rs`
