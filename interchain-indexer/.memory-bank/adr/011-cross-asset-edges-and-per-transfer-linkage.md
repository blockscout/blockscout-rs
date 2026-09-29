# ADR-011: Cross-Asset Stats Edges And Per-Transfer Asset Linkage

**Date:** 2026-09-15

**Authors:** @EvgenKor

## Context

ADR-004 modeled a bridged-token movement as one asset moving from a source
chain to a destination chain: `stats_asset_edges` carries a single
`stats_asset_id`, and `crosschain_transfers.stats_asset_id` links a transfer to
the one asset both of its token endpoints are assumed to name. That assumption
holds for every bridge deployed so far — AMB/Omnibridge and Avalanche ICTT are
both lock/mint: the token that leaves one chain is (by construction) the same
logical asset that arrives on the other.

The xDai bridge (`indexer/xdai/`) breaks the assumption. It is a **converting**
bridge: it locks an ERC-20 reserve (DAI, or USDS via Home v7's explicit token
field) on Ethereum and credits Gnosis' *native coin* — not a wrapped
representation of the reserve, a different asset entirely. A single-asset edge
cannot represent this without one of two bad outcomes: merging DAI, USDS, and
native xDAI into one `stats_assets` row (three assets pretending to be one), or
refusing the transfer (losing the data). Neither is acceptable, and the owner's
call is that DAI, USDS, and xDAI are three assets by design — no single stats
row should claim to answer "total stable value bridged" (that is a separate,
not-yet-built peg/group layer, explicitly out of scope here).

`stats_assets` and `stats_asset_tokens` are not the problem: `stats_asset_tokens`
already partitions chain-local tokens correctly (`PRIMARY KEY (stats_asset_id,
chain_id)`, `UNIQUE (chain_id, token_address)`), and nothing about "one asset,
many chain-local tokens" needs to change. The problem is narrower: the *edge*
and the *transfer link* assume the route is always self-to-self.

## Decision

### 1. The edge becomes binary

```sql
ALTER TABLE stats_asset_edges
  ADD COLUMN src_stats_asset_id BIGINT NOT NULL REFERENCES stats_assets(id) ON DELETE CASCADE,
  ADD COLUMN dst_stats_asset_id BIGINT NOT NULL REFERENCES stats_assets(id) ON DELETE CASCADE;
-- stats_asset_id dropped; new PK:
PRIMARY KEY (src_stats_asset_id, dst_stats_asset_id, bridge_id, src_chain_id, dst_chain_id)
```

`crosschain_transfers.stats_asset_id` splits the same way into
`src_stats_asset_id` / `dst_stats_asset_id` (both nullable — identity can be
partially or fully unresolved, exactly as the single column was).
`stats_assets` and `stats_asset_tokens` are **untouched**: no `bridge_id` is
added to `stats_assets`, and both existing constraints on `stats_asset_tokens`
survive. Every mirror-linkage edge and transfer satisfies
`src_stats_asset_id = dst_stats_asset_id`, so this is a superset of the old
shape, not a replacement of its meaning.

### 2. The indexer declares linkage per transfer; the projection never infers it

```sql
CREATE TYPE transfer_asset_linkage AS ENUM ('mirror', 'conversion');
ALTER TABLE crosschain_transfers ADD COLUMN asset_linkage transfer_asset_linkage; -- nullable
```

Every transfer constructor states `asset_linkage` explicitly — `mirror` for
AMB and Avalanche ICTT (both lock/mint), `conversion` for xDai. This is a
deliberate move away from inferring identity purely from token equality:
ADR-004's union-find is sound evidence *for lock/mint*, where "same token
address" and "same asset" coincide by protocol design, but it is not evidence
at all for a bridge that mints a different asset on purpose. Only the indexer
that emitted the transfer knows which protocol shape produced it; the
projection layer must not guess.

`NULL` means "the indexer has not yet stated the linkage" and defers the
transfer from stats projection **unconditionally**, added as one more clause
to the shared `transfer_identity_ready_condition`
(`stats/indexed_chains.rs`) — the same predicate live projection and startup
backfill already share, so they cannot drift. The declaration is
**write-once**: `crosschain_transfers_on_conflict`'s
`COALESCE(crosschain_transfers.asset_linkage, EXCLUDED.asset_linkage)` lets a
`NULL` be filled by a later flush but never lets a stated value change. No
per-bridge capability method, no config field, and no default value for
`asset_linkage` were added — considered and explicitly rejected, because a
config-level default would reintroduce exactly the kind of protocol-blind
inference this decision moves away from, and a wrong default is
indistinguishable from a correct one until the asset graph is already
corrupted.

There is no compile-time guarantee that every constructor states a linkage —
`crosschain_transfers::ActiveModel` is a public generated struct with public
fields, so a direct struct literal with `..Default::default()` still compiles.
Two things approximate the guarantee instead: `new_transfer(linkage)`
(`interchain-indexer-entity/src/manual/`) is the correct, obvious way to start
every transfer's `ActiveModel`, and the single production write chokepoint,
`flush_to_final_storage`, `debug_assert!`s that `asset_linkage` was `Set` (so
the omission fails loudly in the first test that exercises it) and in release
warns + counts + leaves the value unset (so the row defers rather than being
silently stamped with a guessed `mirror`).

### 3. `mirror` keeps ADR-004's union-find; `conversion` resolves each side independently

`ensure_asset_for_transfer` becomes a dispatcher on `asset_linkage`:

- **`mirror`** — today's function, byte-for-byte unchanged: both endpoints
  reconcile to one asset via `merge_assets`, exactly as ADR-004 Decision 2
  describes.
- **`conversion`** — each endpoint's token is looked up (and created, if
  absent) into its **own** asset, independently. No cross-side link is ever
  attempted and no `asset_has_token_on_chain` probe runs, so this path is
  collision-free by construction: the PK `(stats_asset_id, chain_id)` cannot
  collide on a freshly created asset, and `UNIQUE (chain_id, token_address)`
  cannot fire because the lookup always precedes the insert.
- **partial endpoints under `conversion`** — a narrowing of **ADR-004 Decision
  1**: when only one endpoint is known, there is no second asset to name.
  Writing `src = dst` would assert precisely the identity `conversion` denies,
  so the transfer **defers** (a distinct
  `STATS_TRANSFERS_DEFERRED_TOTAL{reason="conversion_endpoint_unresolved"}`),
  never a skip. Unreachable today (xDai always populates both endpoints) but
  given explicit, tested behaviour rather than left to accident.
- **conversion amounts never fall back across sides** — a further narrowing.
  ADR-004's edge accumulation substitutes the opposite side's raw amount when
  the chosen side's is `NULL`; that is a fee-difference approximation for
  `mirror` (same asset on both sides) and pure fiction for `conversion`
  (records one asset's quantity as another's). A conversion transfer whose
  `amount_side` amount is absent instead defers
  (`reason="amount_side_missing"`), `stats_processed` staying `0`.

ADR-004 Decision 2's union-find is **narrowed, not overturned**: it still
grows one asset across chains for every `mirror` transfer, exactly as before.
What changes is scope — it is no longer applied to evidence (a conversion
transfer's endpoints) that was never a claim about shared identity in the
first place.

### 4. Two contradiction guards, both warn-and-continue

Both catch a state where an earlier declaration and the resolved asset graph
disagree — never an `Err` (the maintenance transaction also carries cursor
writes; ADR-004's error-handling stance on refusals applies unchanged):

- **`conversion_self_asset`** — a `conversion` transfer whose two endpoints
  resolve to the *same* asset (an earlier `mirror` declaration, or bad token
  data, already merged them). Recorded as a self-edge and left in place; it is
  a contradiction to investigate, not a correction to apply.
- **`cross_asset_edge_collapsed`** — a `mirror` merge folds a pre-existing
  cross-asset edge (a genuine `conversion` route between the same two
  components) into a self-edge. `merge_assets` handles the PK collision this
  produces without violating uniqueness, and counts it.

Both guards run **after** the batch's transitive merge remap, immediately
before edge writes — not inline during resolution — because resolving them
inline would make firing depend on batch boundaries: a conversion pair
resolved to two assets, followed *in the same batch* by a mirror transfer that
merges those two components, would otherwise slip past both guards (the first
sees two distinct assets; the second, live inside `merge_assets`, cannot see
an edge that has not been written yet). A diagnostic whose firing depends on
batch boundaries is not a diagnostic.

### 5. The read path unions two directional projections

`bridged_tokens_query.rs`'s innermost aggregate becomes a `UNION ALL` of "this
asset as a destination on the focal chain" and "this asset as a source on the
focal chain" instead of a single `CASE WHEN` per row. For all existing
(mirror) data this reproduces the old query's groups exactly, since
`src_stats_asset_id = dst_stats_asset_id` for every row.

## Alternatives Considered

### An asset-group / peg layer over `stats_assets`

Would let one query answer "total stable value bridged" across DAI, USDS, and
xDAI. **Rejected for now**: the owner's call is that this is not needed yet,
and this decision does not foreclose it — the two-asset edge is what makes
such a layer expressible later, on top of, not instead of, this model.

### A per-bridge `supports_cross_asset_transfers()` capability / config default for `asset_linkage`

Would let the projection infer `mirror` vs `conversion` from bridge
configuration instead of a per-transfer field. **Rejected**: this reintroduces
protocol inference into a layer that should not have it, and a bridge can in
principle emit both shapes (a converting bridge's degenerate same-asset leg is
still `mirror`) — the linkage is a property of the transfer, not the bridge.

### Recording a collision-blocked `mirror` merge as a cross-asset edge

Considered when scoping `refused_chain_collision`: instead of refusing a merge
that would place two tokens of one chain into one asset, record it as a
`conversion`-shaped edge to preserve the counts. **Rejected**: it turns the
one guard this model deliberately keeps (the chain-collision refusal) into a
silent auto-conversion, exactly the kind of unverified protocol inference this
decision exists to avoid. The protocol really is lock/mint in that case; the
flag would be lying.

### Several ICTT `TokenRemote`s of one Home on the same remote chain

Out of scope, tracked separately (`tmp/tasks/stats-asset-representation-layer/`).
The cause is structural — "is a representation of" is not transitive while
union-find requires transitivity — and belongs to a representation-layer
decision, not this one. Declaring such a transfer `conversion` to get past the
same-chain refusal was explicitly rejected: the protocol is lock/mint, and the
indexer cannot know the chain is already occupied by another remote.

## Consequences

### Positive

- A converting bridge's two (or more) distinct assets are each represented
  correctly, with no `stats_assets` row pretending to hold more than one
  token identity.
- The indexer contract stays the single place protocol knowledge lives
  (ADR-004 Decision 4), extended by one field or per-transfer construction,
  not a new type of config or capability surface.
- `mirror` data is provably unaffected: the migration's one in-place `UPDATE`
  sets `src = dst = old value` and `mirror` for every existing row, and the
  read path, the union-find, and the edge accumulation all specialize back to
  their pre-ADR-011 behaviour when `src == dst`.
- Both contradiction guards make a declaration/graph disagreement
  diagnosable via a counter, closing a pre-existing gap where the two
  mapping-conflict skips in `ensure_asset_for_transfer` had no counter at all.

### Negative

- `merge_assets`'s edge fold is materially more complex: a loser/winner
  repoint on two columns instead of one produces collision shapes (a direct
  cross-asset edge between the two merging components collapsing to a
  self-edge) that could not occur before.
- The down migration is lossy once `conversion` rows exist: two source assets
  reaching one destination asset collapse onto the same restored single-asset
  key and must be summed, not picked. Its supported use is `just
  migrate-fresh`, not a partial rollback of a running system.
- A wrong declaration is silent in both directions: `conversion` stated as
  `mirror` reproduces the exact DAI/USDS-merged-into-one-asset bug this
  decision fixes; `mirror` stated as `conversion` prevents an asset from ever
  growing across chains via that bridge. Mitigated by the two contradiction
  guards and the compile-adjacent constructor/chokepoint pair, not eliminated.

### Neutral

- `stats_asset_id` (singular) is retired from the schema but its role is
  unchanged in spirit: `src_stats_asset_id` is what an old-binary reader would
  have seen for any existing (mirror) row, and the down migration restores it
  that way.
- No API or proto change. `stats_asset_id` in the bridged-tokens response
  keeps its existing meaning and place in the pagination cursor.

## References

- ADR-004 (stats observability horizon and asset union-find) — Decision 2 is
  narrowed by this ADR, not overturned; Decision 1 is narrowed for
  partial-endpoint conversion transfers.
- `tmp/tasks/stats-asset-model-multi-token/` — task analysis, implementation
  plan, and selected solution (`solution_6.md`) for this work.
- `.memory-bank/gotchas.md` — "Stats Asset Mapping Conflicts Merge; Only
  Same-Chain Collisions Skip" (rewritten for two asset columns), the
  `..Default::default()` omission hazard, the AMB `replace_existing` +
  `conversion` interaction, the codegen PK-tuple reordering.
- `.memory-bank/runbooks/runtime-verification.md` — canary A qualified with
  `asset_linkage = 'mirror'`; the unclassified-rows operational query.
