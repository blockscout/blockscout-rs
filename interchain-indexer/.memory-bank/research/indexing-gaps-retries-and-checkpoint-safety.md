# Indexing Gaps, Retries, and Checkpoint Safety

## Scope

This note documents the current failure and checkpoint semantics of the two
implemented bridge indexers:

- Avalanche Teleporter / ICM + ICTT
- AMB / Omnibridge

It covers the shared `LogStream`, layered RPC retries, protocol-specific
post-fetch processing, `MessageBuffer` cursor derivation, catchup completion,
restart and shutdown behavior, and the actual runtime status of
`indexer_failures`.

The question is whether a block or event can be skipped without a later replay,
and whether `indexer_checkpoints` can cover work that was not processed
successfully. "Gap" below means a block or event that is no longer guaranteed
to be requested after a later checkpoint is persisted.

This note intentionally does not propose fixes, estimate production incident
frequency, or validate deployed configuration, RPC behavior, logs, or alerts.
Intentional filter/config exclusions are described separately from accidental
gaps.

## Short Answer

Yes. Both indexers can advance beyond blocks whose logs were fetched
successfully but whose receipts, blocks, parsing, correlation, or buffer
mutations failed afterward.

The core boundary is that `LogStream` treats `eth_getLogs: Ok(...)` as
successful scan completion. It yields the returned logs but receives no
acknowledgement from the consumer. When the consumer asks for the next item,
`LogStream` advances the in-memory range even if protocol processing failed.
Both indexer loops log such errors and continue.

`MessageBuffer` checkpoints only know about blocks recorded by successful
`buffer.alter()` calls. Cursor calculation deliberately bridges gaps between
known cold blocks as "scanned but empty". A post-`getLogs` failure leaves no
hot barrier, so a later successfully persisted item can move the checkpoint
across the failed block. Catchup has an additional direct checkpoint writer
that marks the historical scan complete based on raw log scanning, not
successful consumer processing.

By contrast, an `eth_getLogs` error does not skip its starting point. The
provider performs a finite retry burst, then `LogStream` starts another burst
after `poll_interval`, with no total attempt limit. A permanent deterministic
error therefore stalls that scan direction indefinitely rather than creating a
gap.

`indexer_failures` is schema only. No runtime path inserts, selects, retries, or
deletes its rows.

## Why This Matters

`indexer_checkpoints` is the restart source of truth for both indexers, but its
effective meaning is narrower than "every covered block was processed without
errors". It represents:

- blocks attached to buffer entries that maintenance classified as hot or cold;
- inferred empty gaps between those known blocks;
- catchup completion reported by `LogStream`.

It does not represent a durable per-range processing acknowledgement. Once
`realtime_cursor` has increased or `catchup_max_cursor` has decreased, the
monotonic upsert rules do not allow a later discovered failure to move the
cursor back.

The distinction matters operationally:

- some RPC failures are safe stalls;
- some empty/filtered ranges only cause replay and extra load;
- post-fetch failures can become permanent data gaps while the indexer remains
  `Running`;
- maintenance is transactional for work it knows about, but cannot protect
  work that never reached `touched_blocks`.

## Source-of-Truth Files

### Shared ingestion and persistence

- `interchain-indexer-logic/src/log_stream.rs`
  - catchup/realtime range movement, `eth_getLogs` retry loop, direct catchup
    completion write
- `interchain-indexer-logic/src/provider_layers.rs`
  - finite per-request RPC pool retries, timeout, backoff, node selection
- `interchain-indexer-logic/src/message_buffer/buffer.rs`
  - `alter()` mutation and block recording order, maintenance task
- `interchain-indexer-logic/src/message_buffer/buffer_item.rs`
  - `touched_blocks` and version state
- `interchain-indexer-logic/src/message_buffer/maintenance.rs`
  - hot/cold classification and transactional commit
- `interchain-indexer-logic/src/message_buffer/cursor.rs`
  - gap bridging and hot barriers
- `interchain-indexer-logic/src/message_buffer/persistence.rs`
  - checkpoint reads/upserts inside maintenance
- `interchain-indexer-logic/src/database.rs`
  - `get_checkpoint()` and direct `mark_catchup_complete()`
- `interchain-indexer-logic/src/indexer/cleanup_guard.rs`
  - task cleanup without a final buffer drain
- `interchain-indexer-server/src/server.rs`
  - shutdown calls to `indexer.stop()`

### Avalanche

- `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
  - stream consumption, receipt/block fetches, transaction dispatch, event
    parsing, buffer mutations, lifecycle
- `interchain-indexer-logic/src/indexer/avalanche/settings.rs`
  - polling and batch defaults

### AMB / Omnibridge

- `interchain-indexer-logic/src/indexer/amb/indexer.rs`
  - stream consumption, receipt fetch, per-transaction dispatch, lifecycle
- `interchain-indexer-logic/src/indexer/amb/events.rs`
  - event parsing, in-memory `messageHash` correlation, queue draining,
    Omnibridge transfer extraction
- `interchain-indexer-logic/src/indexer/amb/consolidation.rs`
  - source-led and destination-only persistence/finality
- `interchain-indexer-logic/src/indexer/amb/abi.rs`
  - configured address/topic filter
- `interchain-indexer-logic/src/indexer/amb/version.rs`
  - supported AMB/mediator event grammar
- `interchain-indexer-logic/src/indexer/amb/settings.rs`
  - polling, batch, and receipt-concurrency defaults
- `interchain-indexer-logic/src/indexer/evm/log_stream_builder.rs`
  - shared AMB checkpoint restoration and initial cursors
- `interchain-indexer-logic/src/indexer/evm/receipt_fetch.rs`
  - all-or-nothing receipt/block fetch
- `interchain-indexer-logic/src/indexer/evm/transaction_grouping.rs`
  - unordered transaction grouping
- `interchain-indexer-server/src/indexers.rs`
  - AMB chain construction and configured start block

### Schema

- `interchain-indexer-migration/src/migrations_up/m20251030_000001_initial_up.sql`
  - `indexer_checkpoints` and `indexer_failures`
- `interchain-indexer-entity/src/codegen/indexer_checkpoints.rs`
- `interchain-indexer-entity/src/codegen/indexer_failures.rs`

## Key Types / Tables / Contracts

- `LogStream`
  - produces `Vec<Log>` from merged catchup and realtime streams
  - has no consumer acknowledgement or failed-range callback
- `PoolConfig`
  - configures finite retries for one RPC request burst
- `MessageBuffer<T>::alter`
  - mutates protocol state, then records `(chain_id, block_number)`, then
    increments the item version
- `BufferItem<T>::touched_blocks`
  - the only block evidence consumed by maintenance cursor calculation
- `CursorBlocksBuilder`
  - classifies known blocks as hot barriers or cold progress
- `indexer_checkpoints`
  - per `(bridge_id, chain_id)` monotonic catchup/realtime boundaries.
    `catchup_min_cursor` and `catchup_max_cursor` are designed to delimit the
    *unscanned* interval, so catchup completion means the two have met — but
    `catchup_min_cursor` is written as constant `0` and read nowhere
    (`message_buffer/persistence.rs:394`, `database.rs:2512`), so today the lower
    bound lives only in `started_at_block` from config. `finality_cursor` is
    likewise constant `0`. Full cursor semantics and diagram:
    `message-lifecycle.md`, Layer 1 step 2.
- `indexer_failures`
  - intended failed interval ledger; currently unused at runtime
- `pending_messages`
  - durable cold-tier serialized `BufferItem`, used to resume incomplete
    message assembly
- AMB `message_hash_lookup`
  - in-memory `messageHash -> Key` correlation map
- AMB `pending_message_hash_events`
  - in-memory queue for confirmations/signature events observed before their
    source request

The current implicit contract is:

> A checkpoint is safe for protocol state that successfully reached the buffer
> and was represented in the maintenance plan. It is not proof that every RPC
> log in the covered block range reached that state.

## Step-by-Step Flow

### 1. Startup and scan boundaries

For each `(bridge_id, chain_id)`, the indexer restores
`realtime_cursor` and `catchup_max_cursor`. Both stored boundaries are used
inclusively as the next `LogStream` start/end, so the boundary block itself is
replayed. Blocks strictly beyond an already moved boundary are not.

Without a checkpoint, both implementations fetch the current latest block
`N`, start realtime at `N`, and start catchup at `N - 1`, down to the configured
contract start block.

### 2. Raw log fetching

For one provider call, the layered transport has a finite retry count. The
default `retry_count = 5` means up to six dispatch attempts: the initial
attempt plus five retries. Defaults also include a 30-second HTTP timeout and
exponential backoff starting at 200 ms and capped at 5 seconds.

If `eth_getLogs` still fails, `fetch_logs()` returns an error to `LogStream`.
`LogStream` sleeps for `poll_interval` and repeats without a total attempt
counter:

- catchup preserves the exact `[from_block, to_block]`;
- realtime preserves `from_block`; `to_block` is recalculated from the latest
  height and may grow until capped by `batch_size`.

Defaults are 1,000 blocks per range with a 10-second Avalanche poll interval
and a 500-ms AMB poll interval. These settings are configurable.

### 3. The missing processing acknowledgement

After `eth_getLogs` succeeds, `LogStream` yields the returned vector. Yielding
does not carry a success/failure result back from the indexer. On the next poll:

- realtime sets `from_block = to_block + 1`;
- catchup sets `to_block = from_block - 1`.

This movement is independent of receipt fetching, parsing, buffer mutation, or
database persistence performed by the consumer.

### 4. Protocol processing

Both indexers fetch receipts and blocks after the raw range was yielded:

- Avalanche builds one all-or-nothing receipt/block map for the batch with
  `try_collect()`.
- AMB uses the shared EVM helper, also based on `try_collect()`.

One exhausted RPC error, `Ok(None)` receipt/block, missing receipt block number,
or invalid/missing block data aborts the receipt-fetch phase. The provider's
finite transport retries may already have run, but neither indexer requeues the
range. Their outer loops log the processing error and request the next
`LogStream` item.

### 5. Buffer block attribution

Successful handlers call `buffer.alter(key, chain_id, block_number, mutator)`.
`alter()` applies the fallible mutator first. Only after it returns `Ok` does it
record the block in `touched_blocks` and increment the version.

Consequences:

- a handler error before `alter()` produces no cursor barrier;
- a mutator that changes `inner` and then returns an error leaves a mutation
  without block attribution/version increment;
- a later successful event can exist in the same or a later block while the
  failed event remains invisible to cursor calculation.

### 6. Maintenance and cursor derivation

Maintenance snapshots hot entries independently of batch processing. It
classifies each entry:

- entries remaining in memory contribute hot barrier blocks;
- finalized entries contribute cold blocks;
- stale incomplete/partial entries are first serialized to
  `pending_messages`, then contribute cold blocks.

Offload, canonical flush, stats projection, pending cleanup, and checkpoint
upsert occur in one DB transaction. A DB failure rolls all of them back, and
the maintenance loop tries again. This is a real safety property for entries
present in the snapshot.

Cursor calculation then walks known cold blocks and bridges any interval with
no known hot block. That assumption is unsafe for post-fetch failures because a
failed block appears in neither set.

Checkpoint upserts are monotonic:

- `catchup_max_cursor = LEAST(existing, new)`;
- `realtime_cursor = GREATEST(existing, new)`.

### 7. Catchup completion

When raw catchup scanning reaches the configured genesis/start block,
`LogStream` directly calls `mark_catchup_complete()`. It lowers
`catchup_max_cursor` to `genesis_block - 1` without waiting for:

- successful handling of the yielded batches;
- maintenance to persist current hot entries;
- a final buffer drain.

This write is outside the maintenance transaction. If a checkpoint row exists,
failed historical processing or an intervening shutdown can therefore be
covered permanently.

There is one limited safe case: if catchup observed logs, no checkpoint row was
ever created, and no realtime work created one concurrently,
`mark_catchup_complete()` performs an update that affects no row. A restart
then scans again. Empty catchup may create a row with the realtime start cursor,
but that boundary is inclusive and is replayed.

That safe case is **incidental and fragile**: it rests entirely on the checkpoint
row not existing yet. Today the row is created only by the first successful
maintenance pass that has something to write (`upsert_cursors`) — so the property
silently disappears for any bridge/chain with early buffer activity, and it would
disappear altogether if anything else began seeding the row at startup. It should
not be relied on as a safety mechanism.

## Invariants

### Confirmed safety properties

- `eth_getLogs` errors do not move the affected in-memory range.
- `get_block_number` errors in an already running realtime stream only delay
  polling.
- Provider retries for one RPC burst are finite, but outer `eth_getLogs`
  retries are unbounded.
- Maintenance persistence and its checkpoint update commit or roll back
  atomically for the buffer state included in that plan.
- Hot and unchanged non-stale buffer entries remain cursor barriers; stale
  incomplete entries become cold only after durable `pending_messages`
  offload.
- A maintenance DB failure does not advance its cursor and is retried on the
  next tick. This is a per-transaction property only: it does not prevent the
  post-recovery leap described under "Database outage followed by recovery".
- Stored cursors are monotonic in their scan direction.
- Empty or fully filtered batches with no buffer mutation generally cause
  stale checkpoints and replay after restart, not skipped data.

### Assumptions that are not enforced

- Successful `eth_getLogs` is assumed to be a complete response.
- Every yielded log range is assumed to be successfully consumed.
- A gap without `touched_blocks` is assumed to contain no relevant failed
  work.
- Catchup scan completion is assumed to imply processing completion.
- State needed after checkpoint advancement is assumed to be represented by a
  `BufferItem`; AMB's in-memory-only correlation maps violate this assumption.
- Latest-tip logs are assumed canonical; no confirmation depth or reorg
  reconciliation enforces this.

### Properties that do not hold

- A checkpoint does not prove that every covered block was parsed successfully.
- A processing error does not automatically change the indexer state to
  `Failed`.
- A failed range is not persisted for later replay.
- A planned stop does not drain current batches or run final maintenance.
- `finality_cursor` does not provide reorg/finality protection; current writers
  set it to zero.

## Failure Modes / Observability

### Scenario matrix

| Scenario | Immediate behavior | Automatic replay | Permanent gap risk |
|---|---|---:|---:|
| `eth_getLogs` returns `Err` | Same starting point retried indefinitely | Yes | No; direction can stall forever |
| Realtime `get_block_number` returns `Err` | Wait and poll again | Yes | No |
| Startup checkpoint/latest-block RPC fails | Indexer task enters `Failed` | No internal supervisor | Backlog; external restart can recover |
| Receipt/block RPC exhausts retries or returns `None` | Processing batch aborts; loop continues | No | Yes |
| Event decode/handler/buffer restore fails | Error logged; failed event/batch has no barrier | No | Yes |
| Maintenance DB transaction fails | Transaction rolls back; next tick retries | Yes | No for known plan state |
| DB outage spanning several batches, then recovery | No cursor movement during the outage; on recovery one maintenance pass bridges the whole window | No | Yes — highest severity, see below |
| Crash/stop before hot state is persisted | Tasks are aborted without drain | Via checkpoint only | Usually replay; yes with premature checkpoint/race |
| Reorg after checkpoint advancement | No rollback or overlap scan | No | Yes; orphan rows and missed replacements |
| RPC returns `Ok` with omitted logs | Treated as successful empty/partial range | No | Yes; not detectable by current code |

The last row is an external-provider correctness assumption rather than a
locally detectable error.

### Common permanent-gap mechanisms

#### Post-`getLogs` batch failure

Receipt/block fetching can spend all provider retries and still fail. Since
`LogStream` has already yielded the range, the next request advances. A later
cold item can bridge the missing block and make the gap durable.

An RPC `Ok(None)` is especially important: it is a successful transport
response, so the provider retry layer does not retry it before the indexer
converts it to an error.

#### Cursor snapshot races with a still-running batch

Protocol code groups transactions in `HashMap`s and does not guarantee block
order. Maintenance runs concurrently on a separate task. A later-block
finalized item can be included in a maintenance plan while an earlier
transaction from the same fetched range has not yet been dispatched. With no
hot marker for the earlier block, cursor gap bridging can cross it. A crash,
planned stop, or subsequent processing error in that window makes the gap
permanent.

#### Direct catchup completion

Catchup can be marked complete after raw scanning even if one or more yielded
historical batches failed processing or successful mutations still exist only
in RAM.

#### Stop and cleanup

Both indexers abort their indexing and buffer maintenance tasks. The server
calls `stop()` after the launcher exits, but neither implementation waits for
the active batch or performs a final maintenance pass. Conservative checkpoints
normally cause replay, but not when one of the premature-advancement mechanisms
already covered the work.

#### Database outage followed by recovery

This is a composite of two individually documented behaviours whose combination is
worse than either, and it is the most severe mechanism in this note.

While PostgreSQL is unavailable:

- maintenance transactions fail and roll back, so **no cursor moves** — the
  documented safety property holds;
- but both streams keep polling and yielding, because `eth_getLogs` does not need
  the database;
- any `alter()` for a key not already in the hot tier fails, because
  `get_mut_or_default()` → `restore()` performs a `get_pending_message` read
  (`message_buffer/buffer.rs:83-130`). So a subset of batches fails while others —
  those touching only hot entries — succeed and record `touched_blocks` normally.

When the database returns, maintenance snapshots the buffer and derives cursors
from `touched_blocks`, **bridging the gaps between them in a single pass**
(`message_buffer/cursor.rs:209-260`). The blocks of every batch that failed during
the outage are in neither the cold nor the hot set, so the cursor leaps across the
entire outage window at once.

Two things make this worse than an isolated post-`getLogs` failure:

- the window can contain arbitrarily many batches, so one recovery can bury a large
  contiguous region;
- it presents as a *single* successful maintenance cycle after an incident that
  operators are already treating as resolved.

It also shows why "a maintenance DB failure does not advance its cursor" must not be
read as a coverage guarantee: it protects the *transaction*, not the *blocks the
transaction never learned about*.

#### Reorgs

Realtime reads the RPC latest tip with no confirmation delay, overlap, stored
block hash, removed-log handling, rollback, or canonicality check. A
replacement event at an already covered height is not guaranteed to be
requested, and canonical rows produced from orphaned logs are not removed.

### Avalanche-specific behavior

- Receipt/block collection is all-or-nothing for the entire yielded batch.
  One failure occurs before any handler runs.
- After receipts are fetched, the indexer iterates an unordered transaction
  map. Any timestamp, block/log metadata, Teleporter ABI decode, blockchain ID
  resolution, cold-buffer restore, ICTT parse, or handler error aborts the
  remaining batch.
- The outer run loop only logs `"failed to process Avalanche log batch"` and
  continues. It does not requeue the batch or mark the indexer `Failed`.
- Logs without `transaction_hash` are silently excluded while grouping.
- A selected Teleporter log without `topic0` is warned about but returns
  `Ok(())` without a buffer mutation.
- Sender-side receipt parsing treats any receipt log without `topic0` as an
  error. Decode errors, multiple/mismatched ICTT source transfers, and
  receiver-side duplicate/mismatched outcomes can abort handling.
- `MessageExecuted` mutates `source_chain_is_unknown` and `execution` before
  fallible receiver-side ICTT parsing. If parsing fails, `alter()` does not
  record the block or increment the version even though the live inner value
  was already changed.
- Configured bridge filters (`process_unknown_chains`, `home_chain_id`,
  configured contracts/signatures, and `started_at_block`) intentionally
  exclude events before the buffer. Those are coverage boundaries, not
  accidental gaps. When all events are filtered, the usual effect is checkpoint
  staleness and replay after restart.

### AMB / Omnibridge-specific behavior

- The shared receipt helper uses all-or-nothing `try_collect()`. One
  receipt/block failure aborts the whole yielded batch before dispatch.
- After receipt fetching, errors are even more local and easier to hide:
  `dispatch_transaction()` logs each handler error, continues through the
  receipt, and returns `Ok(())`.
- AMB ABI/header validation, missing metadata, cold-buffer restore, and
  `alter()` errors therefore leave no durable failed-event marker.
- `TokensBridgingInitiated` and `TokensBridged` are receipt-local enrichment.
  Their decode/type mismatches are silently skipped and returned as `None`.
  The surrounding proxy event can still be successfully buffer-touched and
  checkpointed, producing a canonical message without the missing transfer
  side. A destination-only execution is final even without that transfer.
- `message_hash_lookup` and `pending_message_hash_events` are in-memory
  `DashMap`s. They are neither serialized into `pending_messages` nor rebuilt
  from stored source requests at startup. After restart, confirmations or
  `CollectedSignatures` for an already checkpointed source request are queued
  without a known key. The source request will not necessarily be replayed to
  drain them, so confirmations or `ReadyToClaim` progress can be lost.
- Queued hash events do not call `alter()` until correlation succeeds, so their
  blocks are not hot barriers.
- Queue draining removes the entire pending entry before sequential fallible
  `alter()` calls. An error partway through draining discards the unapplied
  remainder.
- Source request handling performs one `alter()` for the request and a second
  for the optional transfer side. If the second call fails, the first mutation
  can still make the block checkpointable without its transfer.
- Only the configured AMB/mediator addresses, supported ABI versions, and
  registered event grammar are in scope. Mediator `started_at_block` is not
  used; the chain scan starts from the `amb_proxy` start block. Missing
  chain/provider/contract config can remove a side during construction. These
  are configuration/coverage boundaries rather than retry failures.
- Some subscribed mediator events are not standalone persistence inputs:
  token events enrich proxy events from the same receipt, while
  `NewTokenRegistered` and `FailedMessageFixed` currently fall through the
  dispatcher without stored semantics.

### Runtime visibility

- Raw `eth_getLogs` failures are logged on every outer cycle; the scan direction
  can remain stalled while the indexer status stays `Running`.
- Post-fetch errors are logged per batch, transaction, or event, but are not
  represented in a durable table.
- Batch-processing errors handled inside the run loop do not increment the
  indexer's fatal task error counter or set state to `Failed`.
- Buffer maintenance errors increment
  `BUFFER_MAINTENANCE_ERRORS_TOTAL`.
- `BUFFER_CURSOR` exposes the cursor produced by maintenance but does not prove
  full range processing.
- There is no built-in range backlog/gap reconciliation using
  `indexer_failures`.

## Edge Cases / Gotchas

- A permanent `eth_getLogs` error is a livelock/stall, not a hole. There is no
  adaptive range splitting even though `log_stream.rs` contains a TODO for bad
  block isolation.
- Catchup and realtime are separate streams merged together. A
  range-specific catchup error can leave realtime progressing, and vice versa;
  a provider-wide outage usually affects both.
- Empty successful ranges advance only the in-memory `LogStream` position.
  Without a buffer event, maintenance may have nothing from which to derive a
  checkpoint, causing repeat scans after restart.
- Checkpoint boundary blocks are replayed inclusively. This protects the
  boundary block itself, not failed blocks that a later boundary has crossed.
- `LEAST`/`GREATEST` protects cursor monotonicity; it does not validate that the
  proposed cursor is correct.
- `indexer_failures.attempts`, `reason`, and block interval columns have no
  runtime producers or consumers.
- Buffer mutation is not transactional in memory: a fallible mutator can modify
  `inner` before returning an error, while `record_block()` and `touch()` are
  skipped.
- **Realtime range identities never repeat.** Realtime advances
  `from_block = to_block + 1` on every poll (`log_stream.rs:216`), so consecutive
  realtime scans produce adjacent-but-never-identical `[from, to]` intervals.
  Catchup intervals, by contrast, are aligned to `batch_size` and repeat exactly on
  replay. Anything that tries to deduplicate or account for failed ranges by exact
  interval identity therefore works for catchup and grows without bound for realtime;
  coalescing must treat *adjacent* intervals as mergeable, not only overlapping ones.
- **The consumer cannot name the range it is processing.** `LogStream` yields a bare
  `Vec<Log>`, so an indexer that wants to record a failed interval has no way to
  learn `[from_block, to_block]` for the batch it just failed on. This is the
  structural reason `indexer_failures` has no producer today, independent of any
  missing logic.
- Compact `i64` message keys are derived from only eight bytes
  (Teleporter ID prefix; AMB full-ID hash prefix). Collisions are low
  probability but cannot represent two independent native IDs under the same
  `(message_id, bridge_id)` primary key.

## Change Triggers

Update this note when any of the following changes:

- `LogStream` gains consumer acknowledgement, range retries, splitting, or a
  durable failed-range path
- `indexer_failures` gains runtime reads/writes or is removed
- `catchup_min_cursor` gains a real writer, or anything begins seeding the
  `indexer_checkpoints` row at startup (this retires the incidental safe case
  under "Catchup completion")
- receipt/block fetch semantics or retry policies change
- either indexer changes whether processing errors propagate, stop the stream,
  or are retried
- `MessageBuffer::alter()` mutation/block/version ordering changes
- hot/cold cursor derivation or gap bridging changes
- catchup completion moves into the maintenance acknowledgement path
- shutdown begins draining active work and running final maintenance
- AMB hash correlation becomes durable or rebuildable
- confirmation depth, `finality_cursor`, or reorg reconciliation is implemented
- another bridge indexer starts using this shared pipeline

## Open Questions

There are no unresolved source-code questions required to establish the current
gap and checkpoint semantics.

Operational questions remain outside this note:

- how often a PostgreSQL outage has coincided with active indexing, which
  determines whether the recovery-leap mechanism has already produced gaps;
- which retry/poll/batch overrides are deployed;
- how frequently the identified branches occur;
- whether production monitoring detects cursor stalls or missing ranges;
- whether any configured RPC can return successful but incomplete log results;
- whether existing indexed data already contains gaps or orphaned reorg rows.
