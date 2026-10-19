# ADR-007: The Scan Floor Is Reconciled Against The Checkpoint, Not Against `bridge_contracts`

**Date:** 2026-08-05

**Authors:** @EvgenKor

**Status:** Accepted — with a stated expiry condition, see *Consequences → This
decision expires*.

## Context

A `(bridge, chain)` pair has one configured scan floor, derived by `ChainPlan`
(ADR-005: `amb_proxy` minimum for AMB, minimum over all entries otherwise).
`indexer_checkpoints.catchup_min_cursor` stores it so the progress endpoint can
report `catchup_blocks_remaining` without reading config or an RPC.

Lowering a floor must lower the stored value. The cursor-maintenance writer
cannot do it — `upsert_cursors` applies `GREATEST`, which only raises — so a
startup pass owns the downward direction.

The original pass reconstructed the **previous run's** floor from
`bridge_contracts.started_at_block`, in order to act only on a detected
transition. That worked for an in-place edit and failed for a change to the
identity set:

1. A pair completed catch-up at floor `1000`, so `catchup_min_cursor = 1000`
   and `catchup_max_cursor = 999`.
2. Config gains an entry — a second deployment, or an earlier AMB proxy version
   — with `started_at_block = 500`. The old entry stays.
3. The new `(address, version)` has no stored row, so the derived previous floor
   is `None` ("unknown"), which is deliberately not treated as a change.
   Nothing is lowered.
4. The same startup's `upsert_bridge_contracts` writes the new rows. From now on
   the derived previous floor equals the configured floor, forever.

The scan itself was never wrong — `LogStream.genesis_block` comes from config,
so the reopened range is scanned. The report was permanently wrong: with
`lo = max(catchup_min_cursor, start_block) = 1000` and
`catchup_max_cursor = 999`, the endpoint answers `scan_complete = true`, `0`
remaining and 100% for the entire duration of the rescan and after it.

Two mitigations existed and neither reached this path.
`bridges_pending_contracts_upsert` withheld a bridge's contract rows to preserve
evidence, but only when reconciliation *failed*; here it *succeeded* at deciding
to do nothing. And the ordering rule ("reconcile before the upsert") held, yet
bought nothing, because the evidence was already absent rather than overwritten.

The deeper problem is that `bridge_contracts` is a proxy. The quantity the fix
needs is the pair's previous floor; the table stores per-contract values whose
identity set can change underneath the derivation.

## Decision

**Compare the configured floor against `catchup_min_cursor` and lower the stored
value whenever configuration sits below it.** Unconditionally, for every planned
pair, on every startup. `bridge_contracts` is not read.

This is exact rather than a proxy because of a property of the current design:

> `catchup_min_cursor` is not a scan frontier. It never advances with progress —
> the cursor-maintenance writer always supplies `0` and relies on
> `GREATEST(existing, 0)` purely to preserve whatever is there. Exactly two
> writers move it: `seed_catchup_floor` (raise-only) and `lower_catchup_floor`
> (lower-only).

So the stored value *is* the previous run's configured floor. Comparing against
it needs no history, no evidence window, and no notion of a "previous run".

The decision lives in SQL. `lower_catchup_floor` is
`SET catchup_min_cursor = $new WHERE catchup_min_cursor > $new`, so calling it
unconditionally already encodes the raise-guard and the no-op case; re-running it
on a correct pair writes nothing. `decide_floor_reconciliation` and
`derive_prev_floor` are deleted, and so are `bridges_pending_contracts_upsert`
and the startup coupling it required.

`catchup_max_cursor` is deliberately untouched. A completed catch-up already left
it at `old_floor - 1` (`mark_catchup_complete`), which is exactly where the
reopened descending scan resumes; a catch-up still in progress has it above the
old floor and simply keeps walking past it. **Lowering a floor therefore fetches
exactly `[new_floor, old_floor - 1]` — the newly widened range, and nothing
else.** No already-scanned block is re-fetched, which is the whole reason
`catchup_max_cursor` must not be reset along with the floor: resetting it to
`realtime_cursor` would replay the entire history instead.

Disabled bridges are reconciled too: a bridge can be lowered while disabled and
re-enabled later, nothing indexes it in the meantime, and
`enumerate_indexing_targets` still excludes it from the endpoint.

A failed write stays a `warn`. It is not a lost change any more — the next
startup re-enforces the agreement, because the pass asserts an invariant instead
of detecting a transition.

## Alternatives Considered

### Alternative 1: Withhold the contracts upsert when the previous floor is unknown

Extend `bridges_pending_contracts_upsert` to also withhold a bridge whose
derived previous floor read as `None`, so the next startup could still see the
old value.

Rejected: it deadlocks. The new identity only ever gets a stored row *from* that
upsert, so `None` persists, the bridge is withheld again, and the pair is never
reconciled and never gets its contract rows. The mitigation's precondition is
that the evidence exists; here the missing evidence is the trigger.

### Alternative 2: Persist the pair's floor in its own column

Add `indexer_checkpoints.configured_floor` (or equivalent), write it every
startup, and reconcile against it. This is the correct long-term shape and is
what a bidirectional catch-up will require.

Rejected **for now**, not on merit: it needs a migration, and the task this
branch implements is explicitly constrained to add none. Nothing about
Alternative 2 is precluded — see below.

### Alternative 3: Also lower `catchup_max_cursor` to the old floor

Suggested during review as the way to bound the rescan. Rejected as unnecessary
and slightly harmful today: after a completed catch-up the cursor is already
`old_floor - 1`, so the write is a no-op at best and re-scans one extra block at
worst. It becomes necessary only in the bidirectional case described below.

## Consequences

### Positive

- The reported floor and the configured floor agree after the first restart
  following any floor change, whichever way it was expressed.
- Self-healing: the invariant is re-enforced on every startup, so any past
  divergence is corrected regardless of how it arose.
- The ordering hazard is gone. The old doc comment read "MUST be called BEFORE
  `upsert_bridge_contracts` … reordering breaks detection with no test failure
  unless the ordering itself is asserted". There is no order left to get wrong,
  and a test asserts the inverse property.
- One fewer startup coupling: a failed checkpoint write no longer withholds
  `bridge_contracts` rows, which are also a diagnostic surface.
- ~150 lines and a proxy concept removed.

### Negative

- **This is a workaround licensed by a current invariant, not a general
  solution.** Its correctness rests entirely on `catchup_min_cursor` being a
  stored floor rather than a frontier. That property is not enforced by a type
  or a constraint — only by the fact that no writer advances it.
- The pass no longer distinguishes "configuration changed" from "stored value
  diverged for some other reason". Both are corrected identically. That is the
  intent, but it does mean the logs no longer report a transition, only the
  write.

### This decision expires

**When catch-up becomes bidirectional, this function must be replaced, not
extended.**

If `catchup_min_cursor` becomes the ascending frontier, then
`configured_floor < stored` is true whenever the ascending walk has made any
progress — so an unconditional lower would fire on *every* startup and reset that
walk each time. Not a one-off rescan: a loop. The naive guard "only lower when
catch-up is still in progress" is exactly the broken branch.

The replacement is Alternative 2: persist the floor in its own column, separate
from the frontier, and reconcile against that. A lowered floor is then applied by
lowering the stored floor **and** setting `catchup_max_cursor` to
`old_floor - 1`, which confines the rescan to precisely the newly opened range.

Whoever does that must also make the write's failure fatal for the pair. Today a
failed write is a `warn` because the descending scan takes its floor from the
config value handed to `LogStream.genesis_block` and ignores
`catchup_min_cursor` entirely, so the cost is a misreported percentage and no
lost data. Under a bidirectional indexer the identical failure would silently
drop the newly opened range instead.

### Neutral

- `ChainPlan::floor_contracts` survives with one consumer (`start_block()`).
  It remains the single place the per-protocol floor rule is expressed, which is
  ADR-005's point.
- `bridge_contracts` returns to being purely diagnostic — see the gotcha
  "`bridge_contracts` Is Only A Diagnostic Proxy For Runtime Membership", which
  this decision restores rather than contradicts.

## References

- ADR-005 — the floor rule and the one-scanner-per-pair invariant
- `interchain-indexer-server/src/indexers.rs` — `reconcile_catchup_floors`
- `interchain-indexer-logic/src/database.rs` — `lower_catchup_floor`,
  `seed_catchup_floor`
- `interchain-indexer-logic/src/indexer/progress.rs` — the
  `max(catchup_min_cursor, start_block)` read-side guard this feeds
- `.memory-bank/gotchas.md` — "`catchup_min_cursor` Is A Stored Floor, Not A
  Frontier"
