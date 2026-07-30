# ADR-004: Stats Eligibility From An Observability Horizon; Asset Identity As Union-Find

**Date:** 2026-07-27

**Authors:** @EvgenKor

## Context

Bridged-token stats projection had two independent defects that turned out to
share a root cause.

**Fragmented assets.** `stats_assets` is a surrogate-keyed logical asset;
`stats_asset_tokens` maps chain-local tokens to it with PK
`(stats_asset_id, chain_id)` and `UNIQUE (chain_id, token_address)`. Projection
resolved an asset from whatever endpoints a transfer happened to carry, creating
a singleton asset from a single known endpoint. When two singletons formed
independently from opposite-side observations of the same pair, a later complete
transfer found its two endpoints mapped to two different assets and took a
`warn + skip` branch — permanently. Production hit exactly this via AMB
destination-only rows during a reindex.

Deferring projection until both endpoints are known (the first direction we took)
fixes that case but not the general one: an asset spanning **three or more**
chains can still split with fully complete transfers on fully indexed chains
(`A→B` forms one component, `C→D` another, a later `B→C` bridges them and is
skipped). Asset identity is a connected-component problem discovered one edge at
a time, so it needs a union operation regardless of how partial data is handled.
ICTT routed remote-to-remote transfers make this reachable.

**Missing data for unconfigured chains.** A chain a bridge has no contract on is
never scanned for that bridge's events, yet rows referencing it exist (the
resolver auto-creates the `chains` row to satisfy the FK under
`process_unknown_chains`). Such a message can never be observed from both sides.
Judging stats eligibility by protocol status alone silently discarded all of it:
for Avalanche an outgoing message to an unconfigured chain stays `Initiated`
forever, because the confirming events occur there. Product wanted this data
available behind an opt-in read filter rather than lost.

The two defects looked unrelated because in the current protocols they are
disjoint. AMB has exactly two chains, both always configured, and cannot derive a
peer token from one side — so its incompleteness is always a temporary ordering
artifact. Avalanche has unconfigured chains as the norm but carries both token
addresses in a single `send` event — so its blocker is finality, never endpoint
completeness. Any rule keyed on protocol would encode this accident. The service
is meant to host arbitrary generic bridges, so it must not.

Related: ADR-002 (write-side unknown-chain policy — untouched here) and ADR-003
(per-side event reconstruction with nullable sides, whose "never mirror an
unknown side" rule this decision preserves).

## Decision

### 1. Eligibility follows an observability horizon

The stats layer decides countability from one protocol-agnostic question: *can
the missing evidence still arrive?* It can exactly when the chain that would
produce it is indexed **by that bridge** — i.e. that bridge has a configured
contract there.

- missing token endpoint, counterpart chain indexed → defer;
- missing token endpoint, counterpart chain unindexed → commit to what is known;
- missing destination confirmation, destination chain indexed → defer;
- missing destination confirmation, destination chain unindexed → count now.

The same rule governs `stats_messages` / `stats_messages_days` and
`stats_asset_edges`, so an opt-in read filter behaves coherently across the
message-path and bridged-token endpoints.

Membership is per bridge, not a global union, because all three additive
aggregates are bridge-qualified. It is derived from the **in-memory
configuration**, never from the `bridge_contracts` table: startup backfill runs
before `upsert_bridge_contracts`, so a DB-derived set would be stale exactly when
backfill needs it. A chain becoming configured therefore reclassifies existing
aggregate rows with no migration and no reprojection — nothing about
"unindexed" is ever persisted per row. `dst_chain_id IS NULL` defers
unconditionally: there is no chain to test.

Consequence accepted deliberately: a transfer counted because its confirmation
could never arrive stays counted, even if it later turns out it never completed.
That inaccuracy is confined to a slice clients opt into.

### 2. Asset identity is union-find, with eager merge

A transfer is an edge joining two token vertices; an asset is a connected
component. Resolving a transfer's asset is a `union`:

- neither vertex mapped → create an asset, link both;
- one mapped → link the other into it;
- both mapped to the same asset → no-op;
- both mapped to different assets → **merge** them;
- a merge that would place two different tokens of one chain in one asset →
  refuse, warn, mark processed. This is the only genuine conflict.

Merge is eager — rows are repointed and the losing `stats_assets` row deleted —
rather than a `merged_into` pointer, so no read path dereferences anything and
pagination stays untouched. Winner is the component with more linked tokens, ties
by lower id. Mutation order is fixed (tokens → edges → transfers → metadata →
delete) because the FK cascades make an early delete silent data loss, and
validation is a separate pass so a refusal never half-mutates. Every conflict is
a refusal, never an `Err`: the merge runs inside the shared maintenance
transaction that also carries cursor writes, so an error would roll that back
every cycle.

### 3. Counting and identity are separate concerns

`stats_processed` guards **counting** only: additive, exactly once, never reset,
never reversed. Asset **identity** maintenance is idempotent and may re-run for an
already-counted transfer whose endpoints changed. This is what makes late repair
affordable without touching markers or replaying aggregates.

Filling a missing endpoint always arrives via a flush of that canonical key
(`flush_to_final_storage` is the only production writer of the token-address
columns), so running identity maintenance for every transfer of every flushed key
catches every relink. No marker column and no migration are required.

### 4. Indexer contract instead of protocol branching

Indexers, not the stats layer, own protocol knowledge:

1. if a transfer side can be derived from events on any single chain the bridge
   indexes, fill it — do not leave it NULL merely because the counterpart chain's
   event was not observed;
2. chain-id columns are always known;
3. `status` reflects only the protocol lifecycle — never encode "the other side is
   unobservable" into it;
4. persist partial canonical rows and let the stats layer decide countability.

### 5. A configuration change never retroactively reinterprets indexed history

*(Added 2026-07-28.)* The observability question is asked by exactly one method,
`IndexedChains::may_observe(bridge_id, chain_id)` — *may this bridge observe
events on this chain?* — used identically by projection and by the read filter.
Its non-obvious answer is the unifying constraint: **editing `bridges.json` must
not change the meaning of data already indexed**, neither by committing an
unconfirmed backlog into append-only aggregates nor by hiding complete historical
rows from the API. Three cases:

- **Bridge removed from config** → `true` (permissive). `upsert_bridges` never
  deletes a `bridges` row and no read or projection path filters on
  `bridges.enabled`, so the rows survive and only their classification could
  change. `false` would make the removed bridge's whole unconfirmed backlog
  countable — reachable on startup backfill, which scans all `stats_processed = 0`
  rows regardless of which indexers run — and would hide its completed rows.
- **`enabled == false`** → the bridge's contracts still define its set. An
  operational pause is not a statement about observability.
- **Chain added to a bridge** → the one intended exception: the default view
  widens and previously deferred rows become countable, with no migration. That
  direction is the desired behaviour, and it is safe because it never reverses a
  count and never removes a row from a response.

A bridge **present** in the config with **no contracts** is the deliberate
opposite of a removed one: it observes nothing, so it is restrictive on both
sides, and startup warns about it. Absent means decommissioned; present-and-empty
means misconfigured. Collapsing the two breaks one guarantee or the other.

An earlier read-side design had a second method answering `false` for an absent
bridge. That was an artifact of a SQL shape which enumerated only bridges present
in the map; the shape now carries an explicit permissive arm. One question, one
answer, two callers.

### Scope boundaries

Cumulative amounts stay approximate: folding two edge rows rescales the loser's
sum to the winner's decimals and keeps the winner's `amount_side`, leaving only a
source-vs-destination fee difference. No per-side sum columns. Multi-hop
transfers are counted per hop, not per user action. Already-split production
assets are not repaired in place; the supported recovery is a clean reindex,
though a split heals if a new countable transfer bridges the components.

## Alternatives Considered

### Defer projection until both token endpoints are known, and nothing more

**Cons:** leaves three-or-more-chain assets permanently split, and its indefinite
deferral of permanently one-sided transfers discards exactly the unindexed-chain
data the read filter is meant to expose. Retained inside this decision as the
"counterpart chain is indexed" branch.

### Keep AMB destination-only buffer entries non-final until the source is seen

**Cons:** conflates protocol terminality with indexer observation completeness in
`is_final`, is protocol-local, and leaves other persistence paths free to
reintroduce singleton assets. Useful only as optional AMB hardening.

### Persist an "unindexed" classification per row

**Cons:** a chain becoming configured would then require a data migration and
reclassification pass. The chain-id columns already present on every aggregate
make the classification derivable at read time for free.

### Deterministic asset key from an authoritative on-chain registry

Key the asset by a canonical pair discovered from, for example, Omnibridge's
`NewTokenRegistered` (already in the subscribed mediator grammar but unconsumed).
**Pros:** removes the root cause — no incremental discovery, no merges.
**Cons:** per-protocol, requires new event handling and storage, and does not
exist for a generic bridge. Recorded as the long-term direction, not adopted.

## Consequences

### Positive

- A permanently split asset stops being a reachable terminal state.
- One rule covers ordering gaps, unconfigured counterparts, restarts, backfill,
  and later config changes; live projection and backfill share it by construction.
- Unindexed-chain data is projected with real chain ids, so the read side
  includes or excludes it with a plain predicate, and adding a chain to a bridge
  config needs no migration.
- One membership method serves both sides (Decision 5), so the read filter and
  projection eligibility cannot drift apart; there is no second predicate to keep
  in sync.
- No new table; no cursor or sort key changes; pagination semantics untouched.
- Aggregates stay append-only.

### Negative

- Merge is real machinery: a transactional multi-table operation with a
  refusal path and a potentially large batched repoint of
  `crosschain_transfers.stats_asset_id`. Weighted union bounds churn by token
  count, which does not bound transfer volume — instrumented rather than solved.
- Eligibility now depends on configuration, so the set must be threaded
  identically into live projection and backfill; divergence loses rows or makes
  backfill loop. A bridge *declared with no contracts* makes all of its partial
  data countable, so startup warns per bridge and fails on an all-empty config.
  (A bridge *absent* from the config, and a config with no bridges at all, fail
  open instead — Decision 5.)
- Deferred rows are permanent and accumulate at the low end of the id range,
  degrading backfill candidate scans; mitigated with a monotonic cursor and
  phase separation rather than a partial index.
- The opt-in slice mixes confirmed and merely-initiated movements.

### Neutral

- Permanently one-sided transfers whose counterpart chain *is* indexed (AMB
  `messageId` collisions, history older than the configured start block) stay
  deferred forever by design. A marker-zero row with a NULL endpoint is not a
  backfill backlog.
- The pre-existing `Failed AND bridge = AMB` clause in the shared finality
  predicate remains the one piece of protocol knowledge in the stats layer.
  Generalizing it is optional future scope; extending it is not allowed.

## References

- Observability-horizon eligibility and union-find asset merge: commits
  `6f102e15`, `636b9a53`, `cb81d8ea`, `bde5fe7d`
- Read-side `include_unindexed_chains` filter and `has_unindexed_chain` flag:
  commits `883c9efb`, `b0540fc8`, `a7dd0acd`
- Avalanche indexer-contract requirement 1 (incoming ICTT reconstruction),
  which this decision's eligibility rule depends on: commit `9329320c`
- `.memory-bank/gotchas.md` — "Stats Eligibility Is About Observability, Not
  Protocol Terminality"
- `.memory-bank/research/stats-projection.md`,
  `.memory-bank/research/stats-subsystem.md`
- ADR-002 (write-side unknown-chain policy), ADR-003 (nullable transfer sides)
