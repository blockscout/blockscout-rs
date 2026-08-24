# Indexing Concurrency Model and Throughput

## Scope

Covers **how concurrent the indexing pipeline actually is** at every level —
across bridges, across chains of one bridge, across scan directions of one
chain, and within a single batch — plus measured throughput and the
head-of-line blocking that the current shape produces.

Out of scope: what each batch *does* once it is handed to a processor (see
`message-lifecycle.md`), and the failure/retry semantics themselves (see
`indexing-gaps-retries-and-checkpoint-safety.md` and ADR-005). This note is
about scheduling, not about correctness of the data produced.

## Short Answer

Concurrency is real at every level: **between bridges** (one `tokio::spawn` per
bridge, genuinely parallel), **between the chains of one bridge** (one
sequential handler future per chain, joined with `try_join_all`, plus the retry
pass as a sibling future — all cooperatively interleaved inside the bridge's
single task), and **between the two scan directions of one chain**.

The unit of serialisation is therefore one chain, not one bridge. A chain
throttled to a low `max_rps` parks on its own `until_ready()` and its siblings
are polled throughout.

The remaining ceiling is deliberate: a bridge is still **one tokio task**, so a
long *CPU-bound* stretch with no `.await` delays siblings. It is bounded by
construction — `process_batch` awaits an RPC per transaction, so the longest
uninterrupted synchronous stretch is one transaction's ABI decode. True
parallelism (`tokio::spawn` per chain) was rejected: it would open check-then-act
windows in the AMB correlation maps that contain no `.await` and that
cooperative interleaving provably cannot enter.

**Historically** (before `evgenkor/interchain/per-chain-range-drivers`) all of a
bridge's streams were merged into one `SelectAll` and consumed by a loop that
awaited `handle_batch` inline, so one chain's batch blocked every sibling. That
also manufactured `TimedOut` errors on healthy chains — a reqwest timeout is a
tokio timer that elapses whether or not its future is polled — and those fake
failures fed `mark_error`, cooling down nodes that never failed.

## Why This Matters

The driving case is a **deliberately rate-limited chain as a steady state**,
not an incident. Some chains have a single RPC with hard rate limits — NUMINE
via Glacier is the concrete one — and the operator wants to set a low `max_rps`
so that chain syncs slowly *on purpose*. Under the pre-change shape that choice
silently set the pace for every other chain on the bridge: at `max_rps = 1`, a
batch of N transactions holds the whole bridge for ~2N seconds, because each
transaction issues a receipt and a block call and the entire `process_batch` is
swallowed by one inline `await` in `handle_batch`.

The baseline and tuned measurements below are exactly this experiment. Glacier
at `max_rps = 1` put C-Chain and Henesys — chains with no relationship to
NUMINE — at 41-66 s mean gaps; raising Glacier to 20 moved them to 6-7 s.
Raising the limit is a workaround, not a fix: it takes away the operator's
ability to run a chain slowly.

Compounding this, the Avalanche bridge is expected to grow to many subnets.
Under that baseline every added chain lengthened the blocking period for every
existing chain, so throughput per chain degraded as `O(1/N)`. For AMB the chain
count is small but the requirement is the same: a stalled or throttled Gnosis
must not stop Ethereum from being indexed, since messages are assembled
incrementally and one side's events are still useful on their own.

A slow chain must therefore be treated as **healthy and normal**, never as a
fault: no backpressure that accumulates, no stall detector, no liveness check
that could escalate it.

## Source-of-Truth Files

- `interchain-indexer-logic/src/indexer/range_driver.rs` — `RangeDriver::run`:
  `try_join_all` over one sequential handler per chain, raced in a
  `tokio::select!` against the retry future; `RangeProcessor` trait;
  `retry_pending` default implementation
- `interchain-indexer-logic/src/indexer/failure_ledger/mod.rs` — the per-pair
  record epoch that makes `record`/`resolve` safe to interleave
- `interchain-indexer-logic/src/log_stream.rs` — `LogStream::into_stream`
  merges the catchup and realtime sub-streams for one chain via
  `stream::select`; per-batch `sleep(poll_interval)`
- `interchain-indexer-logic/src/indexer/avalanche/mod.rs` — `run()` builds the
  per-chain `Vec<(chain_id, stream)>`; `process_batch` receipt fan-out
- `interchain-indexer-logic/src/indexer/amb/indexer.rs` — same shape for AMB;
  `amb/events.rs` holds the cross-chain correlation maps
- `interchain-indexer-logic/src/indexer/evm/log_stream_builder.rs` — shared
  per-chain stream construction, checkpoint restore, scan-floor seeding
- `interchain-indexer-server/src/indexers.rs` — `spawn_configured_indexers`,
  one indexer per bridge
- `interchain-indexer-logic/src/provider_layers.rs` — per-node `max_rps`
  limiter, `RPC_HTTP_TIMEOUT`, `mark_error` / cooldown
- `scripts/indexing_throughput.py`, `scripts/indexing_coupling.py` — the two
  log analysers used for every measurement below; see "How these measurements
  were taken"

## Key Types / Tables / Contracts

| Element | Role |
| --- | --- |
| `CrosschainIndexer::start` | spawns exactly one tokio task per bridge |
| `RangeDriver<P>` | owns the run loop for a whole bridge; `retry_cursor` is a `run` local, which is what lets N handlers share `&self` |
| `RangeProcessor` | per-bridge trait; every method already takes `chain_id` |
| `LogStream` | one per `(bridge, chain)`, internally two sub-streams |
| `FailureLedger` | `Arc`-shared per bridge; `pairs_with_holes` cache keyed `(bridge_id, chain_id)` |
| `MessageBuffer` | `Arc`-shared per bridge; `DashMap` keyed by message `Key` |

## Step-by-Step Flow

1. `spawn_configured_indexers` builds one indexer per enabled bridge and calls
   `start()` on each. Each `start()` does its own `tokio::spawn`, so **bridges
   are genuinely parallel** on the multi-thread runtime.
2. Inside a bridge, `run()` calls `build_log_stream_for_chain` per chain and
   collects `Vec<(chain_id, stream)>` — the streams are **not** merged.
3. `RangeDriver::run` wraps each chain in its own `while let` handler future and
   joins them with `try_join_all`, raced in a `tokio::select!` against a retry
   future that never completes.
4. Each handler **awaits** `handle_batch` → `RangeProcessor::process` → the
   adapter's `process_batch` before pulling its next item. Within a chain,
   batches stay strictly in arrival order; across chains they interleave at
   every `.await`. `try_join_all` returns on the first `Err`, so an
   unrecordable-failure escalation still fails the whole bridge loudly.
5. `process_batch` groups logs by transaction and fetches receipt + block per
   transaction under `buffer_unordered(receipt_concurrency)` — default 25 in
   both adapters. Each call is throttled by the node's `max_rps`.
6. Successful batches call `ledger.resolve`; failures call `ledger.record`.
   Cursors are **not** derived here — `MessageBuffer` maintenance derives them
   independently from cold/hot block sets.

## Invariants

- **One scanner per `(bridge, chain)`.** ADR-005 and the gotcha of the same
  name: checkpoints, ledger rows, and progress targets are all keyed
  `(bridge_id, chain_id)`, so the unit of scanning must match that key.
- **Cursor derivation is order-independent.** `message_buffer/cursor.rs`
  computes the frontier from cold/hot block sets during maintenance, not from
  batch arrival order. It already tolerates catchup and realtime landing
  interleaved. `MessageBuffer::alter` holds its `DashMap` guard only across
  synchronous work, so it is safe under both interleaving and true parallelism.
- **Gap bridging assumes in-direction sequencing.** The cursor treats gaps
  between cold blocks as "scanned but empty". That is true only because
  batches within one direction of one chain are processed in order. Any change
  that lets batches of the *same* chain and direction land out of order breaks
  it. Concurrency *between* chains does not.
- **Escalation relies on per-chain monotonicity.** `range_driver.rs`'s
  `bail!` path is justified by "realtime is monotone forward per chain, so once
  the driver stops consuming, no buffer entry above the failed interval can
  appear". The claim is per-chain and survives cross-chain concurrency.
- **`FailureLedger::initialize` replaces, it does not merge.** It assigns
  `*pairs_with_holes.write() = cache`. Several drivers sharing one `Arc`
  would wipe each other's entries, and a stale-`false` cache makes `resolve`
  skip the database and leave a hole unresolved forever. One driver per bridge
  calling `initialize(&all_pairs)` once keeps this unreachable.
- **`record` must bump the pair's epoch AFTER its write, not only before.**
  `resolve` snapshots the epoch, then asks the database whether rows remain;
  that `COUNT` runs in its own transaction and cannot see an uncommitted
  `INSERT`. With only a leading bump, a `resolve` that snapshots after it reads
  the already-bumped value, its `COUNT` returns zero, the equality check
  passes, and it clears the pair against a row that commits a moment later —
  stale-`false`, and the hole is then replayed forever with no log line. The
  trailing bump is the one carrying the safety property; `or_insert` also
  repairs an entry a concurrent `resolve` already removed. Pinned by
  `record_bumps_the_epoch_after_its_write_not_only_before`, which the
  `may_clear_pair_*` equality tests cannot catch.
- **AMB correlation state is cross-chain by design.** `message_hash_lookup`
  and `pending_message_hash_events` are shared `DashMap`s: the source chain
  writes, the destination chain reads. They must stay shared under any
  decomposition.

## Failure Modes / Observability

**Symptom of head-of-line blocking:** every stream of one bridge advances in
bursts a few hundred milliseconds wide, separated by tens of seconds of
silence, and clusters of `TimedOut` warnings for *different* chains share a
millisecond with the log line that follows them.

**Diagnosis:** group `scanning … logs` lines by `(bridge, chain, direction)`
and compare the *gap distribution*, not averages. Independent chains interleave
smoothly; a shared blocked task shows near-identical gaps across every stream
of one bridge and near-zero gaps within a burst.

**Not observable in metrics.** These timeouts fail inside `fetch_logs`, which
retries the same range without yielding a batch, so `RangeDriver` never learns
and no `indexer_failures` row is written. Nothing counts them.

## Measurements

Local `just run-dev`, `config/full-mainnet` — bridge 1 = AMB/Omnibridge
(Ethereum 1 + Gnosis 100, unmetered Blockscout nodes at `max_rps = 100`),
bridge 2 = Avalanche ICM/ICTT (C-Chain 43114, NUMINE 8021, Henesys 68414,
public endpoints).

### How these measurements were taken

Reproducible from the repo; two helper scripts do the arithmetic.

**Capture.** `just run-dev` against `config/full-mainnet` and live endpoints,
output to a file. Strip the ANSI colour codes before analysing — `tracing`'s
terminal writer emits them and they break the log regexes:

```bash
just run-dev > run.log 2>&1
sed -E 's/\x1b\[[0-9;]*m//g' run.log > run.clean
```

Take the sample by **line count, not wall time** (`until [ "$(grep -c scanning
run.log)" -ge 300 ]`), so both sides of a comparison contain comparable work
rather than comparable duration.

**Control the starting state.** `just flush-database` before each side. Without
it the two runs start from different checkpoints and scan different block
ranges, which dominates any scheduling effect. Note this also restarts every
chain's catch-up, which is the point: all chains producing batches at once is
the maximum-pressure case for a shared loop.

**Vary only one thing.** Keep the configuration byte-identical between sides and
change only the code, or vice versa. Use a fixture copy of `.env` rather than
editing it:

```bash
sed -E 's#^(INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__MAX_RPS=).*#\11#' \
    .env > /tmp/.env.slow-numine
dotenv -f /tmp/.env.slow-numine run just run > run.log 2>&1
```

Beware that `just run` sets `INTERCHAIN_INDEXER__AVALANCHE_INDEXER__BATCH_SIZE`
as a **command-prefix assignment inside the recipe body**, which overrides the
inherited environment — so exporting that variable before `just` silently does
nothing. To vary it, invoke the binary directly with `dotenv -f .env run env
VAR=... target/debug/interchain-indexer-server`. One probe here was invalidated
by exactly this.

**Throughput and gaps** — `scripts/indexing_throughput.py`:

```bash
python3 scripts/indexing_throughput.py run.clean
```

Groups `scanning … logs` lines by `(bridge, chain, direction)` and reports
batches, blocks, blocks/s, and mean/max inter-batch gap. Read the **gap
distribution**, not the averages: independent chains interleave smoothly, while
chains sharing a blocked task show near-identical gaps across every stream of
one bridge and near-zero gaps inside a burst. `blocks/s` is computed over each
stream's own first-to-last batch, so it is comparable across runs of different
length.

**Coupling — the decisive check** — `scripts/indexing_coupling.py`:

```bash
python3 scripts/indexing_coupling.py run.clean <slow_chain_id> [fast_chain_id ...]
```

Splits each fast chain's gaps into those that overlap a slow chain's batch and
those that do not, and compares the two distributions. Coupled ⇒ the
overlapping gaps are markedly longer. Decoupled ⇒ the columns match. This ratio
is **internal to one run**, so it is valid even when two runs differ in length —
which is why it is the primary evidence rather than raw throughput.

**Choose the slow chain for signal, not for realism.** Throttling NUMINE is the
faithful production scenario but a blunt instrument: early catch-up blocks are
sparse, so even at `max_rps = 1` its `process_batch` has almost nothing to do
and there is little to block with. Throttling C-Chain — dense with Teleporter
events — is the sharp instrument. Run both: the sharp one proves the property,
the faithful one shows the deployment you actually operate.

**Two traps worth naming.**

- *A stream-level stall proves nothing.* Black-holing a chain's RPC (pointing it
  at a non-routable IP) leaves its `LogStream` parked in `fetch_logs`, which
  `SelectAll` simply does not advance — so siblings are unaffected and in fact
  speed up. Head-of-line blocking comes only from chains that successfully fetch
  and then spend wall-clock inside `process_batch`. The same applies to tests:
  inject inside `RangeProcessor::process`, never inside the stream.
- *Don't `tail` a backgrounded run.* Piping through `tail -n` keeps only the tail
  and discards every per-binary `test result:` / scan line you wanted.

### Baseline — 2026-08-19, 250 s window

`pull_interval_ms = 500`, `batch_size = 1000` (default), glacier
`max_rps = 1`, C-Chain and Henesys on the implicit `default_max_rps() = 10`.

| Stream | blocks/s | mean gap | max gap |
| --- | --- | --- | --- |
| 1/1 realtime | 0.1 | 12.0 s | 16.3 s |
| 1/100 realtime | 0.2 | 5.1 s | 10.1 s |
| 2/43114 catchup | 20.1 | 66.4 s | 87.5 s |
| 2/43114 realtime | 1.4 | 50.0 s | 103.1 s |
| 2/68414 catchup | 32.3 | 41.3 s | 69.1 s |
| 2/8021 catchup | 20.9 | 63.7 s | 78.3 s |

`TimedOut` warnings: **11 in 250 s**. Bridge 2 aggregate catchup: ~48 blocks/s.

Note that bridge 1 is unaffected — its gaps are 5–12 s. The pathology is
entirely intra-bridge, and bridge 2 is where it lands because its nodes are
rate-limited.

### After configuration tuning — 2026-08-20, 662 s window

`AVALANCHE_INDEXER__BATCH_SIZE = 200`, glacier `max_rps = 20`, C-Chain and
Henesys `max_rps = 25` (matching the 25-wide receipt fan-out).

| Stream | blocks/s | mean gap | max gap |
| --- | --- | --- | --- |
| 1/1 realtime | 2.9 | 11.7 s | 19.7 s |
| 1/100 realtime | 6.9 | 5.2 s | 14.2 s |
| 2/43114 catchup | 33.0 | 6.1 s | 17.7 s |
| 2/43114 realtime | 30.0 | 6.7 s | 17.2 s |
| 2/68414 catchup | 32.2 | 6.3 s | 26.1 s |
| 2/8021 catchup | 28.1 | 7.2 s | 20.3 s |

`TimedOut` warnings: **0 in 662 s**. Retry passes: 0. Bridge 2 aggregate
catchup: ~93 blocks/s, roughly 1.9× the baseline.

### Stall injection — 2026-08-20, 143 s window

One chain's RPC pointed at a non-routable IP (`http://10.255.255.1:8545`) so
every request hangs to `RPC_HTTP_TIMEOUT`. Same tuned configuration as above.

| Stream | blocks/s | mean gap |
| --- | --- | --- |
| 2/43114 catchup | **241.5** | 0.8 s |
| 2/43114 realtime | 18.4 | 1.9 s |
| 2/68414 (black-holed) | — | 1 batch total |

C-Chain catchup went from 33 blocks/s with three healthy chains sharing the
loop to **241.5 blocks/s** once one sibling stopped producing batches — a 7×
change, with the mean gap collapsing from 6.1 s to 0.8 s. NUMINE's absence from
the catchup column is legitimate: it had reached genesis (`catchup complete`)
before this run.

This is the scaling claim measured directly: per-chain throughput degrades
roughly as `1/N` in the number of *batch-producing* chains on a bridge.

**The nuance this exposed.** A chain stalled at the *RPC* level does **not**
block its siblings today. It stalls inside its own `LogStream` — in
`fetch_logs` or `get_block_number` — and `SelectAll` simply leaves that stream
`Pending` and polls the others. RPC-level stalling is the already-working case.

Head-of-line blocking comes exclusively from chains that successfully fetch and
then spend wall-clock inside `RangeProcessor::process` / `process_batch`, since
that is what the driver `await`s inline. Any test for the decoupling property
must therefore inject its block inside `process`, not inside the stream — a
stream-level stall passes against the sequential code and proves nothing.

### After per-chain concurrency — 2026-08-21

Both sides from a flushed database with the identical fixture (NUMINE at
`max_rps = 1`, C-Chain and Henesys at 25, `batch_size = 200`), so only the code
differs.

| Stream | before | after |
| --- | --- | --- |
| 1/1 catchup (Ethereum) | 275.8 blocks/s | **405.8** |
| 1/100 catchup (Gnosis) | 313.1 | **444.0** |
| 2/43114 catchup (C-Chain) | 115.6 | **249.6** |
| 2/68414 catchup (Henesys) | 71.7 | **114.7** |
| 2/8021 catchup (NUMINE, 1 RPS) | 33.1 | **43.1** |

Max gaps collapsed with them: C-Chain 8.1 s → 3.8 s, Henesys 8.1 s → 4.2 s.
The deliberately throttled chain got *faster* too — it no longer queues behind
its siblings, only behind its own limiter.

### The decisive experiment — a heavy chain throttled to 1 RPS

The NUMINE-slow fixture understates the effect: early catch-up blocks are sparse,
so even at 1 RPS its `process_batch` has little to do. Throttling **C-Chain**
instead — whose catch-up ranges are dense with Teleporter events — gives the
sharp instrument.

| | C-Chain at 25 RPS | C-Chain at 1 RPS |
| --- | --- | --- |
| C-Chain itself | 249.6 blocks/s | **26.0** (10x slower, as configured) |
| Henesys | 114.7 | **114.1** — unchanged |
| Henesys max gap | 4.2 s | **2.7 s** |
| AMB bridge (chains 1 / 100) | 405.8 / 444.0 | 451.4 / 413.3 |

A sibling held its throughput to within noise while the throttled chain dropped
tenfold. That is the requirement, measured on the running service.

A residual coupling remains and is expected: comparing each sibling's gaps that
overlap the slow chain's batches against those that do not gives 1.47 s vs
1.17 s (Henesys). Sub-second, and consistent with the accepted single-task
ceiling — I/O waits decouple, CPU segments do not.

### Rate limits observed

Across all post-change runs: **zero `TimedOut`**, zero manufactured timeouts.
Real quota rejections, however, are visible and are the thing to tune against:

| Node | Configured | Result |
| --- | --- | --- |
| `api.avax.network` (C-Chain) | `max_rps = 25` | clean, no 429 |
| `henesys-rpc.msu.io` | `max_rps = 25` | clean, no 429 |
| `glacier-api.avax.network` (NUMINE) | `max_rps = 20` | **HTTP 429**, 12 in ~66 s |
| `1rpc.io/eth` (Ethereum fallback) | none → default 10 | **JSON-RPC -32001** "usage limit for your current plan", 11 in ~66 s |

Two lessons. First, 20 RPS is above Glacier's quota — and since decoupling makes
a low limit free, a rate-limited chain should simply be configured low rather
than pushed. Second, at measurement time the Ethereum publics in
`config/full-mainnet/chains.json` (`gateway`, `drpc`, `1rpc`) carried no
`max_rps` at all, so they inherited `default_max_rps() = 10`; when the pool
rotated the primary away from the unmetered Blockscout node they immediately
exhausted their plan. That is what the current provider set fixes: every
Ethereum and Gnosis node now carries an explicit `max_rps`.

### Recommended parameters

- **`max_rps` ≥ the receipt fan-out for any chain you want fast**, because
  `process_batch` issues receipts under `buffer_unordered(receipt_concurrency)`,
  default 25; below that the chain throttles itself. Set it *deliberately low* for a chain you want slow —
  that is now free for its siblings.
- **Give every fallback node an explicit `max_rps`.** A fallback is only used
  when the primary rotates, i.e. exactly when the system is already degraded;
  inheriting the default 10 turns a brief rotation into a quota burst.
- **`batch_size` is now a per-chain latency knob**, not a bridge-wide blocking
  bound. Raising it is low-risk after this change and reduces `eth_getLogs`
  calls per block scanned, but it lengthens one chain's own per-batch latency
  and its exposure to `RPC_HTTP_TIMEOUT` on dense ranges. **Not yet measured**
  — an attempted probe was invalid because the `just run` recipe sets
  `BATCH_SIZE` as a command-prefix assignment that overrides the inherited
  environment. Worth testing in staging.
- **`pull_interval_ms` applies to catch-up as well as realtime.** 500 ms is
  fine; the two arguably want different values (catch-up as low as the endpoint
  tolerates, realtime near block time) and cannot be set separately today.

### What tuning did and did not fix

Shrinking `batch_size` shortens the blocking period; raising `max_rps` to match
the receipt fan-out stops each batch from serialising its own RPC calls. Both
help, and together they removed the manufactured timeouts outright.

Neither removed head-of-line blocking. On the baseline the gaps stayed
correlated across the chains of bridge 2 — a chain that genuinely stalled
(endpoint outage, a very dense block range) froze its siblings for the duration.
Config tuning bounded the damage; it did not decouple the chains. Per-chain
handlers did — see [After per-chain concurrency](#after-per-chain-concurrency--2026-08-21).

## Edge Cases / Gotchas

- **`max_rps` must agree with `receipt_concurrency`.** Both adapters default
  the fan-out to 25 while `default_max_rps()` is 10, so an unconfigured node
  parks 15 of every 25 calls against the limiter. That costs no throughput —
  `max_rps` is the bound either way — but it is why peak in-flight requests
  exceed what the endpoint is allowed to serve. Lower `receipt_concurrency`
  rather than raising `max_rps` when the goal is fewer sockets, not more
  blocks.
- **`pull_interval_ms` applies to catchup too**, not just realtime polling — it
  is a flat sleep after every batch in both sub-streams.
- **The retry pass is a sibling future**, not a `select!` branch, so a replay
  pass no longer blocks forward consumption. This needed neither `Arc<P>` nor a
  spawn — an earlier reading of this note claimed it did. The cost is that
  `process()` now runs concurrently with the forward path, which is what makes
  the AMB drain's compare-and-remove and the ledger's record epoch mandatory
  rather than optional.
- **`max_chunks_per_pass` is a bridge-wide budget** deliberately: the cyclic
  `(chain_id, block)` resume cursor exists so one wide interval cannot starve
  the other chains. Any decomposition into per-chain drivers turns it into a
  per-chain budget and multiplies total replay load by the chain count — which
  is the main reason the "one `RangeDriver` per chain" variant was rejected in
  favour of per-chain handlers inside one driver. It now bounds replay RPC load
  per pass rather than a forward-progress pause.
- **AMB's `pending_message_hash_events` is unbounded.** Destination-side events
  queue there until their source request arrives, and only
  `handle_source_request` drains it — no TTL, no cap, no offload. Its occupancy
  is proportional to how far the source chain lags, and this change makes a
  large permanent lag a supported configuration. Watch
  `interchain_indexer_amb_pending_correlation_queue`; a monotonic rise means it
  needs a retention policy, and nothing bounds it today.
- **AMB's drain yields.** `drain_pending_message_hash_events` clones the queue
  entry, `await`s the applies, then `remove`s the entry. Under any cross-chain
  concurrency, events queued during those awaits are removed unapplied. The
  clone-then-remove ordering is required by ADR-005 and must not be inverted;
  the fix is a compare-and-remove.

## Change Triggers

Update this note when:

- `RangeDriver::run`'s loop shape changes, or drivers are decomposed per chain
- a new indexer adapter implements `RangeProcessor` with different blocking
  characteristics
- `buffer_unordered` concurrency or `max_rps` defaults change
- the retry pass moves off the driver's `select!`
- new measurements are taken; keep the baseline table for comparison

## Open Questions

- Should `pull_interval_ms` be split into separate catchup and realtime
  intervals? Catchup wants it as low as the endpoint tolerates; realtime wants
  it near the chain's block time.
