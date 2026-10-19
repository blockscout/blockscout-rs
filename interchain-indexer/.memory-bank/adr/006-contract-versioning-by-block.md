# ADR-006: Contract Versioning Resolved By Block, At Decode Time

**Date:** 2026-08-04

**Authors:** @EvgenKor

## Context

`bridge_contracts` has been shaped for versioned contracts since the initial
migration: `UNIQUE(bridge_id, chain_id, address, version)`, with
`started_at_block` documented in the DDL as "needed to select proper contract
for the concrete block". A contract upgrade is expressed as another config entry
with a higher `version` and the block it takes effect from — and for an AMB
proxy the upgrade happens **behind the same address**.

The runtime could not represent that. Two places dropped the extra entries
before anything could use them:

- `build_amb_chain_configs` took `.find()` per `kind`, so `AmbChainConfig` held
  exactly one `amb_proxy_address` and one `mediator_address`. Whichever version
  the config listed first won; the rest never reached the indexer.
- `AbiRegistry` keyed contracts by `(chain_id, address)` with one ABI each.
  Even had the configs arrived, the second version would have collided with the
  first (`insert_contract` rejected duplicates, so this half failed loudly).

The same `.find()` also made `omnibridge_mediator` mandatory: a chain without
one was skipped entirely rather than indexing messages without transfers.

Related: ADR-005 established that a scan's unit must match the key of the
records describing it. This decision is the same question one level down — the
unit of *decoding* versus the key that identifies an implementation.

## Decision

Carry every configured version through to the registry, and resolve which one
applies **at decode time, by `(address, block)`**.

1. **`AmbChainConfig` holds lists.** `amb_proxies: Vec<AmbContractConfig>` and
   `mediators: Vec<AmbContractConfig>`, each entry carrying `address`,
   `version`, `started_at_block` and `abi`.
2. **`AbiRegistry` stores version windows.** `(chain_id, address)` maps to a
   list of `ContractVersion`s sorted by `started_at_block`; a version is in
   force from its block until the next one's, the last being open-ended.
3. **`resolve_log(chain_id, address, topic, block_number)`** returns
   `Matched`, `NotConfigured`, or `WrongVersion`. `EventContext` carries the
   block number for this reason — it is not diagnostic metadata.
4. **Mediators are optional.** A chain with only AMB contracts indexes messages
   without token transfers. A mediator that *is* configured but has an
   unusable ABI remains fatal: that is a config error, not a choice.

`topic0` cannot substitute for block-based resolution. It hashes the event
signature, which does not include which parameters are `indexed`, so two
versions can share a topic and decode differently.

### The fetch filter stays a union

`filter_for_chain` unions every address and every version's topics — deliberately
wider than any single version window.

Splitting the `eth_getLogs` range into per-version windows was considered and
rejected. It would introduce an invariant that must hold identically in three
places (catch-up, realtime, retry), and a replay whose filter is narrower than
the forward scan re-fetches a recorded hole against the wrong ABI set, finds
nothing, and resolves a hole that was never fixed. That is the exact failure
this branch already fixed twice — once for Avalanche's multi-contract filter,
once for the AMB correlation drain. Decode-time selection costs one lookup and
adds no cross-path invariant.

For versions at *different* addresses, splitting buys nothing at all: a contract
emits no logs before it exists, so an address-union query over the whole range
returns exactly what per-window queries would.

### Dropped logs are counted, not swallowed

A log whose topic belongs to a different version window than its block is
dropped. That is correct only if the configured boundaries are right, so it
increments `interchain_indexer_amb_logs_dropped_wrong_version_total` and warns.
Zero is the expected value; non-zero means a `started_at_block` disagrees with
the chain and real events are being discarded — with no ledger row, because the
blocks *were* scanned.

Ordinary cross-matches are not counted: the filter matches any configured
address crossed with any configured topic, so logs from address A bearing a
topic only address B declares come back routinely. Those are `NotConfigured`.

## Alternatives Considered

### Alternative 1: Per-version `eth_getLogs` windows

Split a scanned range at version boundaries and query each window with only that
version's addresses and topics.

**Pros:**
- The filter is exactly correct per window; no cross-matches to discard.
- Slightly fewer logs transferred.

**Cons:**
- Three call sites must implement the split identically, forever. Retry replays
  a range recorded by the forward path; any divergence resolves holes that were
  never re-read.
- Buys nothing for distinct addresses, which is the only case where "don't query
  a contract before it existed" sounds meaningful.
- Does not remove the need for decode-time `kind` resolution anyway.

Rejected. Correctness lives at decode; the filter's job is to be a superset.

### Alternative 2: Fail startup on a second version of one address

Keep one ABI per `(chain_id, address)` and reject a versioned config outright.

**Pros:**
- Minimal change; converts today's silent drop into a loud failure.

**Cons:**
- Makes the schema's stated purpose unimplementable, so the first real upgrade
  becomes an outage rather than a config edit.

Rejected, though the loud-failure half is retained as a property: two versions
of one address claiming the same `started_at_block` is an error, since their
order would otherwise be undefined.

## Consequences

### Positive

- A contract upgrade is a config edit: add an entry with the new version and the
  block it takes effect from.
- Silent drops at the config→runtime boundary are gone. Every configured entry
  reaches the registry or fails startup.
- AMB-only chains are indexable, which was the stated reason for taking the scan
  floor from `amb_proxy` alone rather than from all kinds (ADR-005, `plan_bridge`).
- Version windows registered at startup are logged, so what the process actually
  believes can be read back instead of inferred from the config file.

### Negative

- A wrong `started_at_block` now discards real events instead of decoding them
  against a neighbouring version's ABI. The metric and warning are the whole
  mitigation — if either is ignored, this is silent loss of exactly the kind
  this work exists to remove.
- Boundaries come from config and are never validated against the chain. Nothing
  detects a boundary that is off by a few thousand blocks except the drop
  counter, and only if the versions' event sets actually differ.
- The registry holds every version's ABI for the process's lifetime. Negligible
  today; worth remembering if version counts grow.

### Neutral

- `AmbSide` is validated to be consistent across a chain's proxy versions. An
  upgrade cannot move a chain from Home to Foreign, so disagreement means the
  config describes two bridges under one chain id.
- `ContractKind` is per version, not per address, because `header_layout` is
  derived from the grammar and is exactly the kind of thing an upgrade changes.

## References

- `interchain-indexer-migration/src/migrations_up/m20251030_000001_initial_up.sql`
  — `bridge_contracts` DDL, the origin of `(address, version, started_at_block)`
- ADR-005 — scanning unit versus record key; the filter-narrowing failure class
- `.memory-bank/gotchas.md` → "A Contract Version Is `(address, block)`, Not `address`"
