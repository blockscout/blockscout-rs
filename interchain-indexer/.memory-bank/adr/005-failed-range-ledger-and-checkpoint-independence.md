# ADR-005: Failed-Range Ledger, Independent of Checkpoints

**Date:** 2026-08-03

**Authors:** @EvgenKor

## Context

`LogStream` treated `eth_getLogs: Ok(...)` as successful scan completion. It
yielded the returned logs but received no acknowledgement from the consumer, so
when the consumer asked for the next item the in-memory range advanced whether or
not protocol processing had succeeded. Both indexer loops logged such errors and
continued.

`MessageBuffer` checkpoints only know about blocks recorded by successful
`buffer.alter()` calls, and cursor derivation deliberately bridges gaps between
known cold blocks as "scanned but empty". A post-`getLogs` failure therefore left
no hot barrier, and a later successfully persisted item could move the checkpoint
across the failed block. The result was a permanent hole while the indexer
reported `Running`.

Two structural facts made this unfixable in place:

- **The consumer could not name the range it failed on.** `LogStream` yielded a
  bare `Vec<Log>`, so an indexer had no `[from_block, to_block]` to record.
- **`indexer_failures` was schema only** — no runtime path inserted, selected,
  retried, or deleted its rows.

Background analysis:
`.memory-bank/research/indexing-gaps-retries-and-checkpoint-safety.md`.

Two constraints shaped the design. First, the mechanism had to be **universal** —
usable by an arbitrary future indexer, not tuned to Avalanche or AMB. Second,
simplicity was explicitly preferred over precision: recording a wider range than
strictly necessary is an acceptable cost, and each indexer may narrow its own
records as far as it finds worthwhile.

## Decision

Record every scanned-but-unprocessed interval in a durable ledger, retry it
forever with a capped backoff, and clear it by set difference when the blocks are
actually reprocessed. Three pieces:

1. **`LogStream` names its output.** It yields
   `LogBatch { from_block, to_block, direction, logs }`.
2. **`FailureLedger`** maintains a disjoint, non-adjacent interval set per
   `(bridge_id, chain_id)` in the existing `indexer_failures` table, with exactly
   two mutations — `record` (union) and `resolve` (set difference) — plus a pure
   `open()` read.
3. **`RangeDriver`** owns the run loop both indexers previously hand-rolled, so
   recording, replay and escalation come from implementing one trait,
   `RangeProcessor`.

**Checkpoints and the ledger are two independent records.** A checkpoint is the
contiguous scan frontier — a restart anchor, certifying that blocks were
*scanned*, not that they were processed correctly. The ledger holds the known
holes below it. Nothing in the shared pipeline reads the ledger; cursor
derivation (`message_buffer/cursor.rs`, `maintenance.rs`) is untouched, and the
two records are joined only by the progress endpoint.

Consequential details, each of which is load-bearing:

- **`record` merges on overlap *or* adjacency** (gap ≤ 1). This is required, not
  an optimization: realtime advances `from_block = to_block + 1` every poll, so
  range identities never repeat, and without adjacency merging a sustained
  realtime failure writes thousands of unmergeable rows per day.
- **`resolve` is set difference**, so partial completion is not a special case —
  a chunk that succeeds is subtracted, splitting the row if it lands in the
  middle. Rows are removed only here; nothing expires them.
- **A failed `record` write is fatal to the stream.** It is the one remaining
  point where a range can be lost. The danger is not the outage itself — while
  the database is down no cursor can move — but the moment it recovers, when
  maintenance bridges gaps and the frontier leaps over intervals that were never
  recorded. Stopping at the first unrecordable failure closes that for every
  *subsequent* batch, because realtime is monotone forward per chain.

  It does **not** close it for the batch in flight. Both adapters process a
  batch's transactions out of order, and maintenance runs concurrently, so a
  later block can already be persisted — and the cursor already advanced past an
  earlier failing block — before the driver learns it cannot record. Stopping
  prevents future work; it cannot retract that. Closing this needs the
  acknowledgement boundary rejected as Alternative 1, so it is carried as a known
  limitation rather than claimed as safe.
- **Proven progress resets the backoff.** A `resolve` split gives remainders
  `attempts = 1` and the parent's `updated_at`. Inheriting the parent's attempt
  count instead makes an hour-long incident take ~22 hours to drain, since each
  pass replays only `max_chunks_per_pass` chunks before the remainder waits out
  the full cap again.
- **Replay uses the indexer's own `batch_size`.** Shared code never talks to an
  RPC; it hands the processor a range and the processor chunks it.
- **No schema change.** `indexer_failures` is used exactly as defined. No
  `given_up_at`, no index, no unique constraint.

`catchup_min_cursor` is put to work by the same change (see the companion
decision below), which is what makes "how much has been scanned" reportable.

### Forward constraint

`catchup_min_cursor` is seeded at startup and lowered by a startup reconciliation
when the configured `started_at_block` drops. A failure of that write is a
`warn`, never fatal — safe only because catch-up is one-directional today and the
downward scan takes its floor from config regardless.

**Whoever makes `catchup_min_cursor` a real scan boundary must first make that
write's failure fatal for the pair.** Under a bidirectional catch-up the same
failure silently drops `[new_floor, old_floor - 1]`, which is exactly the class of
loss this ADR exists to eliminate.

## Alternatives Considered

### Alternative 1: Full acknowledgement protocol in `LogStream`

Have the stream withhold its range advance until the consumer acknowledges
successful processing.

**Pros:**
- Precise: exactly the failed range is retained, with no over-recording.
- No second table; the stream is self-correcting.

**Cons:**
- Reshapes the most-trafficked code in the service and every consumer with it.
- Per-indexer semantics leak into shared code — "processed successfully" means
  something different for Teleporter, ICTT and AMB.
- Rejected on universality: it optimizes precision, which was explicitly the
  cheaper of the two axes.

### Alternative 2: Recorded intervals as checkpoint barriers

Let an unresolved hole hold the cursor back, so the frontier cannot advance
across it and a restart re-scans by streaming.

**Pros:**
- A second recovery path, independent of the retry pass.
- A stalled hole is impossible to miss — the cursor visibly stops.

**Cons:**
- Solves the wrong problem. The original hole was permanent because the range was
  *forgotten*, not because the cursor passed it; a durable ledger already is the
  recovery mechanism, so constraining the cursor on top of it is redundant.
- Couples the ledger into the maintenance transaction, where a ledger conflict
  would roll back the entire flush plus cursor commit.
- One poison range stalls a chain indefinitely.

Rejected. The cost is real and accepted: the retry pass is now the *only*
recovery path, which is why `interchain_indexer_oldest_open_hole_age_seconds`
plus an alert on it is required operational scope rather than polish.

### Alternative 3: A give-up marker (`given_up_at`)

Mark an interval abandoned after N attempts.

**Cons:**
- Requires a migration, and `give_up` on a *partially overlapping* interval needs
  a third interval operation plus two interleaved interval sets per pair that
  must not coalesce.
- Abandoning a hole without saying so is the failure mode worth avoiding most.

Rejected in favour of retrying forever with a capped backoff. Nothing is ever
written off.

## Consequences

### Positive

- A scanned range whose failure **reaches the driver** is durably recorded and
  replayed. That is the mechanism the original silent-loss path needed; whether a
  given failure reaches it is a property of each adapter, not of this decision,
  and is the first thing to check when reasoning about completeness.
- The mechanism is protocol-agnostic: a new indexer gets recording, replay and
  escalation by implementing `RangeProcessor`.
- Checkpoint semantics are now stated rather than assumed — a checkpoint
  certifies scanning, which is what makes the progress endpoint honest.
- Zero migration, so deploy and rollback are unordered; rows already written
  become inert if the binary is reverted.

### Negative

- The retry pass is the only recovery path. If it is wedged, disabled or
  crash-looping, holes persist while the service looks healthy.
- Recorded ranges are wider than necessary — a whole batch by default — so replay
  re-fetches and reprocesses more than the failing blocks. Accepted deliberately.
- A crash between the processing failure and the `record` write is still a silent
  hole. Graceful drain in `stop()` is the cheap follow-up.
- **The batch in flight is not fenced** when a `record` write fails — see the
  Decision section. Narrow window, real hole, and not closable without an
  acknowledgement boundary.
- **`resolve` runs before the work is durable.** A successful `process` means the
  mutation reached `MessageBuffer`; the database write happens later in
  maintenance. A stop in between loses the replayed work *and* the ledger row
  that would have caused another replay — so the ledger is a post-failure work
  queue, not a durable processing acknowledgement, and must not be described as
  one. Catch-up completion has the same shape.
- **A wide interval may drain only partially.** The retry pass rotates its chunk
  window by the row's `attempts` so a deterministically failing prefix cannot
  starve the tail. That sweep is complete only when a failing chunk records one
  range; an indexer narrowing `attributed` per block advances `attempts` faster
  than the window is wide, leaving chunks between the reachable offsets
  un-refetched. Nothing is lost or falsely resolved — the row simply shrinks
  slower. A per-pass counter advancing by the window size would make it
  unconditional.
- **AMB's in-memory correlation drain can still false-clear.** The pending entry
  is removed before the fallible applies, so a failure mid-drain loses the
  remainder; the block *is* recorded, but the replay finds nothing pending,
  succeeds, and resolves the hole while the data is gone. Part of the wider AMB
  restart-durability follow-up.
- **Failures a handler swallows never reach the ledger**, and a replay covering
  them reads as success and resolves the range. Locally detectable malformed
  input (a log without `transaction_hash`, an event without `topic0`, a failed
  token-enrichment decode) is deliberately treated as a data-quality skip rather
  than a failure, because the alternative is retrying ordinary junk forever.
  Consequently `failed_blocks == 0` means "nothing was recorded", not "nothing
  was lost".
- Nothing is ever written off, so a permanently unrecoverable interval keeps the
  staleness metric climbing until someone deletes the row by hand. Truthful, but
  alert fatigue is the risk to watch and the strongest argument for reviving a
  give-up marker.
- The checkpoint row now exists from startup, retiring the incidental "no row ⇒
  catchup completion no-ops ⇒ restart rescans" property.

### Neutral

- `attempts` is approximate by construction (`max + 1` on merge, reset on a split
  that proved progress). Anything needing precision should use
  `created_at`/`updated_at`, which are exact under both operations.
- The healthy path performs zero ledger statements only because of the empty-set
  cache, which assumes **one process indexes a given bridge**. That assumption was
  already implicit in checkpointing; it now has a second consumer.
- No retroactive coverage: the ledger says nothing about ranges scanned before it
  existed.

## References

- `.memory-bank/research/indexing-gaps-retries-and-checkpoint-safety.md`
- `.memory-bank/research/message-lifecycle.md` §2 — cursor semantics
- ADR-001 — message buffer tiered storage (the cursor derivation this leaves
  untouched)
