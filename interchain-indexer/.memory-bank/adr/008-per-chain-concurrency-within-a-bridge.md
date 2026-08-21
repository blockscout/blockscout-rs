# ADR-008: Per-Chain Concurrency Within A Bridge, Cooperative And Single-Task

**Date:** 2026-08-21

**Authors:** @EvgenKor

## Context

`RangeDriver::run` merged every chain's `LogStream` of a bridge into one
`SelectAll` and awaited `handle_batch` inline inside its `tokio::select!`. A
`select!` does not make its branches concurrent — it races them once and runs
the winner to completion — so while any one chain's batch was being processed,
no other chain of that bridge was polled. The retry pass, sitting in the
sibling `select!` arm, had the same property.

The operational consequence is not an incident, it is a **steady state that
cannot be configured**. Some chains have a single RPC with hard rate limits
(NUMINE via Glacier is the concrete one), and the operator wants a low
`max_rps` so that chain syncs slowly *on purpose*. Under the merged loop that
choice paced the entire bridge: at `max_rps = 1` a batch of N transactions held
the driver task for roughly `2N` seconds, because `process_batch` issues a
receipt and a block call per transaction and all of it sat behind one inline
`await`. Chains with no relationship to the throttled one ran at its speed.

Measured, before the change: Glacier at `max_rps = 1` put C-Chain and Henesys
at 41–66 s mean inter-batch gaps; raising Glacier to 20 moved them to 6–7 s.
Raising the limit is a workaround that removes the operator's ability to run a
chain slowly at all. Separately, black-holing one chain's RPC took C-Chain
catch-up from 33 to 241 blocks/s purely because a sibling stopped producing
batches — per-chain throughput degraded roughly as `1/N` in the number of
batch-producing chains. The Avalanche bridge is expected to grow to many
subnets, so that is a scaling wall, and for AMB the same requirement holds with
two chains: a throttled Gnosis must not stop Ethereum, since messages are
assembled incrementally and one side's events are useful on their own.

Full analysis, invariants and both measurement series:
`.memory-bank/research/indexing-concurrency-model.md`.

## Decision

Stop merging. `RangeDriver::run` takes `Vec<(chain_id, stream)>`, wraps each
chain in its own sequential handler future, and joins them with
`futures::future::try_join_all`. The retry pass becomes a **sibling future**
raced in one `tokio::select!` rather than a branch that runs to completion.
Everything stays inside the bridge's existing single `tokio::spawn`.

Load-bearing details:

- **Within a chain, nothing changes.** Each handler is a `while let` loop that
  awaits `handle_batch` to completion before pulling its next item, so
  in-direction arrival order is byte-for-byte what it was. Only ordering
  *across* chains is relaxed. This is required, not incidental: cursor
  derivation bridges gaps between cold blocks as "scanned but empty", which is
  only true because batches of one chain and direction are processed in order.
- **`retry_cursor` moves out of the struct into a `run` local.** It was the
  only `&mut self` borrow in the type; removing it is what lets N handler
  futures share `&self` with no `Arc`, no `Clone` and no `'static`.
- **Cooperative, not parallel.** The unit of serialisation becomes one chain
  instead of one bridge. A throttled chain parks on its own
  `node.limiter.until_ready()` — a per-node limiter, so an await-point yield
  that affects nobody else — and its siblings are polled throughout.
- **`try_join_all` preserves loud failure.** It returns on the first `Err`, so
  an unrecordable-failure escalation still fails the whole bridge into
  `CrosschainIndexerState::Failed`. `stop()` and `CleanupGuard` are untouched
  because there is still one task and one `indexing_handle`.
- **A slow chain is healthy.** It produces batches slowly or not at all and
  never returns an error, so it never escalates. No stall detector, watchdog,
  per-chain timeout or liveness probe may be added on top of this: a chain at
  `max_rps = 1` is supposed to look inactive, permanently.

Three interleavings that this concurrency would otherwise open are closed as
part of the same change, because each is a silent data loss:

- `drain_pending_message_hash_events` compare-and-removes only what it applied,
  so an event another chain queued during its `.await`s survives. The
  clone-then-remove *ordering* mandated by ADR-005 is unchanged; only the
  removal's conditionality.
- Both `message_hash_lookup` removal sites use the same compare-then-remove
  form.
- `FailureLedger` carries a per-pair record epoch so `record` and `resolve` for
  one `(bridge_id, chain_id)` can interleave — which the sibling retry pass
  makes possible. The epoch is bumped on **both** sides of the write, and the
  trailing bump is the one that carries the safety property: `resolve`'s `COUNT`
  runs in its own transaction and cannot see an uncommitted `INSERT`, so a
  leading-only bump is already inside a racing `resolve`'s snapshot and the
  equality check passes.

## Alternatives Considered

### Alternative 1: One `RangeDriver` per chain, joined at the call site

Conceptually tidier — `RangeDriver` is a driver of one scan frontier, and
ADR-005 keys everything `(bridge_id, chain_id)`.

**Pros:**
- Matches the record key exactly.
- Leaves the processors `Clone + 'static`, pre-plumbed for a spawned model.

**Cons:**
- **Buys no additional parallelism.** `try_join_all` over N drivers in one task
  is the *same* cooperative interleaving as N handler futures in one driver —
  one task, one core, interleaving at the same `.await` points.
- Turns `max_chunks_per_pass` from a bridge-wide budget into a per-chain one,
  multiplying replay RPC load by the chain count against the same rate-limited
  endpoints. That is the "adding chains must not degrade existing chains"
  failure mode arriving through the back door.
- Degenerates ADR-005's cyclic `(chain_id, block)` resume cursor, and requires
  one `FailureLedger` per driver — its `initialize` *replaces* rather than
  merges its cache, so a shared `Arc` would leave entries stale-`false` and a
  hole permanently unresolvable.

Rejected: pays an ADR rewrite and an `O(N)` replay-load regression for
organisation, and for spawnability that no measurement justifies.

### Alternative 2: `tokio::spawn` per chain, with a supervisor

True parallelism across cores.

**Pros:**
- Removes the single-core ceiling.

**Cons:**
- **Costs correctness.** The AMB queue-vs-lookup paths contain no `.await`
  between the `get` and the `entry` insert, so cooperative interleaving
  provably cannot enter them but threads can. The failure mode is a silently
  dropped validator confirmation with no ledger row, no log line and no metric.
- Turns `DashMap` shard guards and `parking_lot` locks into real contention
  that blocks tokio worker threads.
- Needs a hand-built supervisor to keep failures loud, plus N-handle tracking
  in `stop()` / `CleanupGuard`. Independent spawns would let one chain die
  silently while the bridge still reports `Running` — strictly worse than the
  status quo.

Rejected for now. The requirement is about a chain *waiting on I/O*, which
cooperative concurrency handles perfectly; no measurement shows a bridge task
saturating a core.

## Consequences

### Positive

- A chain can be given a deliberately low `max_rps` and sync slowly forever
  without affecting its siblings. Measured: throttling C-Chain from 25 to 1 RPS
  dropped it tenfold while Henesys held 114.1 vs 114.7 blocks/s.
- Per-chain catch-up throughput roughly doubled on identical fixtures, and the
  manufactured `TimedOut` bursts — tokio timers elapsing on unpolled futures,
  which then fed `mark_error` and cooled down healthy nodes — disappeared.
- The retry pass no longer starves the forward streams, which retires the
  gotcha "The Retry Pass Starves The Forward Streams".
- `batch_size` stops being a bridge-wide blocking bound and becomes a per-chain
  latency knob.

### Negative / accepted

- **Single-core ceiling per bridge.** All chains plus the retry pass share one
  tokio task, so a long CPU-bound stretch with no `.await` still delays
  siblings. Bounded by construction — `process_batch` awaits an RPC per
  transaction, so the longest synchronous stretch is one transaction's ABI
  decode. Revisit only if a bridge task's busy/wall ratio approaches 1.0, and
  read Alternative 2 first.
- **`process()` now runs concurrently with the retry path.** ADR-005 called
  this a design change of the class it rejected. That was right about the
  mechanism and wrong about the severity: cross-chain concurrency introduces
  the same class regardless, and the two genuinely lossy interleavings it
  exposes are closed above.
- **AMB's `pending_message_hash_events` is unbounded**, and this decision makes
  a large permanent source-chain lag a *supported* configuration rather than an
  incident, so its occupancy matters more than before. Exposed as
  `interchain_indexer_amb_pending_correlation_queue`; a retention policy is
  outstanding.
- Siblings are cancelled mid-`process` on escalation. Not new — `stop()`
  already aborts at an arbitrary await point, and the cursor is derived from
  cold/hot block sets rather than batch completion.

### Verification

The acceptance criterion is not a throughput number, so it is pinned by tests
that inject their delay inside `RangeProcessor::process` — never inside the
stream, because a stream-level stall leaves `SelectAll` simply not advancing
that stream and therefore passes against the pre-change code. All three
(`a_slow_chain_does_not_slow_down_its_siblings`,
`a_chain_blocked_inside_process_does_not_stop_its_siblings`,
`a_blocked_retry_pass_does_not_stop_the_forward_streams`) were confirmed to
fail when the old serialisation is reinstated.
