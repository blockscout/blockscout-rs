# ADR-002: Per-Bridge `home_chain` Filtering for Unknown Chains

**Date:** 2026-02-25

## Context

Unknown-chain filtering needs to be bridge-scoped and simple to reason about.

Requirements:

- strict mode (only configured chains), or
- allow unknown-chain messages only when one endpoint is a designated chain.

## Decision

Move unknown-chain policy into `bridges.json` as:

```rust
home_chain: Option<ChainId>
```

Behavior per bridge:

- `home_chain = None` (or omitted/null):
  - process only messages where both source and destination chains are configured for that bridge.
- `home_chain = Some(id)`:
  - still process all configured↔configured messages;
  - additionally process one-configured/one-unknown messages **only if** `source == id || destination == id`.
- both unknown endpoints are always skipped.

Validation:

- Startup fails if `home_chain` is set but not present in the bridge’s configured contracts.
- `ChainId (u64)` is converted to `i64` internally with range checks.

## Consequences

### Positive

- **Per-bridge control** aligns with operational intent.
- **Simpler mental model**: one field controls unknown-chain policy.
- **Fail-fast validation** prevents silent misconfiguration.

### Negative

- No “allow all unknown chains” mode anymore (intentional simplification).
- Existing configs/tests using old settings must migrate.

### Neutral

- No DB schema changes.
- Filtering still happens before buffering/consolidation.

## Alternatives Considered

1. Keep global settings and pass overrides per bridge.
	- Rejected: duplicate configuration paths.
2. `home_chains: Vec<ChainId>`.
	- Rejected for now: unnecessary complexity for current use cases.
3. Pair whitelist `(chain_a, chain_b)`.
	- Rejected: high config complexity and poor maintainability.

## References

- Plan: `tmp/plans/frolicking-booping-cerf.md`
- Implementation: `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
- Bridge config: `interchain-indexer-server/src/config.rs`
