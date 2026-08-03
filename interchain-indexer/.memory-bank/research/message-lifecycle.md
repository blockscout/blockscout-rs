# Interchain Message Lifecycle

## Scope

This note covers the end-to-end runtime lifecycle of an interchain message,
from log ingestion through final persistence, checkpoints, stats projection
handoff, and token enrichment kickoff.

The note is structured as two layers:

- **Layer 1 (Generic pipeline):** The protocol-agnostic infrastructure that any
  `CrosschainIndexer` implementation feeds into. Covers LogStream, buffer
  mutation, the `Consolidate` contract, maintenance pipeline, checkpoint/cursor
  semantics, and downstream hooks.
- **Layer 2 (Avalanche reference realization):** How the Avalanche indexer
  concretely fulfills the generic contract. Serves as both Avalanche
  documentation and a reference example for future indexer implementations.

Future indexers should get their own separate research notes covering only their
protocol-specific layer, referencing the generic layer documented here.

This note intentionally does **not** cover:

- stats projection internals (see `stats-projection.md`, `stats-subsystem.md`)
- token enrichment internals (see `token-info-service.md`)
- blockchain ID resolution internals (see `avalanche-blockchain-id-resolution.md`)
- API serving layer
- config loading and server startup wiring

## Short Answer

Indexers stream blockchain logs, dispatch them to protocol-specific event
handlers, and mutate entries in a shared `MessageBuffer` via `buffer.alter()`.
Each protocol implements `Consolidate` to define when a buffered entry is ready
for persistence and when it is final. A periodic maintenance loop classifies
every hot-tier entry, flushes consolidated entries to canonical tables, offloads
stale entries to cold storage, projects stats within the same DB transaction,
updates checkpoint cursors, and kicks off token enrichment after commit.

The pipeline is protocol-agnostic from `buffer.alter()` onward. Protocol
specifics live entirely in the indexer's event handlers, domain type, and
`Consolidate` implementation.

## Why This Matters

The message lifecycle spans multiple subsystems (log streaming, buffering,
consolidation, persistence, stats, enrichment) and multiple concurrency
boundaries (per-chain streams, per-key buffer mutations, periodic maintenance).
Understanding the full flow prevents future changes from breaking invariants at
subsystem boundaries — especially the ordering guarantees inside the
maintenance transaction and the cursor advancement rules that ensure safe
restart.

This note is also the primary reference for implementing new indexers: Layer 1
defines what the shared pipeline expects, and Layer 2 shows how a concrete
implementation fulfills that contract.

## Source-of-Truth Files

### Generic pipeline

- `interchain-indexer-logic/src/log_stream.rs`
- `interchain-indexer-logic/src/message_buffer/buffer.rs`
- `interchain-indexer-logic/src/message_buffer/buffer_item.rs`
- `interchain-indexer-logic/src/message_buffer/types.rs`
- `interchain-indexer-logic/src/message_buffer/maintenance.rs`
- `interchain-indexer-logic/src/message_buffer/persistence.rs`
- `interchain-indexer-logic/src/message_buffer/cursor.rs`
- `interchain-indexer-logic/src/stats/service.rs`
- `interchain-indexer-logic/src/indexer/crosschain_indexer.rs`

### Avalanche realization

- `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
- `interchain-indexer-logic/src/indexer/avalanche/types.rs`
- `interchain-indexer-logic/src/indexer/avalanche/consolidation.rs`
- `interchain-indexer-logic/src/indexer/avalanche/blockchain_id_resolver.rs`

## Key Types / Tables / Contracts

### Generic

- `CrosschainIndexer` — trait with `start()`/`stop()` lifecycle contract
- `LogStream` — bidirectional log streaming primitive (catchup + realtime)
- `MessageBuffer<T>` — tiered buffer with hot (DashMap) and cold
  (`pending_messages`) tiers
- `BufferItem<T>` — versioned wrapper: `inner: T`, `touched_blocks`,
  `version`, `last_flushed_version`, `hot_since`
- `Key` — `(message_id: i64, bridge_id: i16)`, compact for FK efficiency
- `Consolidate` trait — `fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>>`
- `ConsolidatedMessage` — `{ is_final, message: ActiveModel, transfers: Vec<ActiveModel> }`
- `CursorBlocksBuilder` — accumulates per-bridge per-chain cold/hot block sets
- `Cursor` — `{ backward: BlockNumber, forward: BlockNumber }`
- `StatsService` — orchestrates stats projection and token enrichment kickoff
- `crosschain_messages` table — canonical finalized messages
- `crosschain_transfers` table — canonical finalized transfers
- `pending_messages` table — cold-tier buffer storage
- `indexer_checkpoints` table — per-bridge per-chain cursor state

### Avalanche-specific

- `Message` — domain type with `send`, `receive`, `execution`, `transfer`,
  `source_chain_is_unknown` slots
- `AnnotatedEvent<T>` — event wrapper adding `transaction_hash`,
  `block_number`, `block_timestamp`, `source_chain_id`, `destination_chain_id`
- `TokenTransfer` — enum: `Sent(src, dst)` | `SentAndCalled(src, dst)`
- `MessageExecutionOutcome` — enum: `Succeeded` | `Failed`
- `BlockchainIdResolver` — Avalanche native blockchain ID → EVM chain ID
- `AvalancheIndexer` — concrete `CrosschainIndexer` implementation

## Step-by-Step Flow

### Layer 1: Generic Pipeline

#### 1. Log ingestion via LogStream

`LogStream` is a reusable bidirectional log streaming primitive. It produces a
merged stream of two scanning directions:

- **Catchup** (backward): fetches historical blocks from `catchup_cursor` down
  to `genesis_block`
- **Realtime** (forward): polls for new blocks from `realtime_cursor` onward

Configurable parameters: `filter`, `batch_size`, `poll_interval`,
`genesis_block`, `realtime_cursor`, `catchup_cursor`, `bridge_id`, `chain_id`.

LogStream is protocol-agnostic. Indexers configure it with their own filters
and cursors.

#### 2. Checkpoint restoration and cursor semantics

On startup, indexers read `indexer_checkpoints` for their `(bridge_id,
chain_id)` pairs to determine `realtime_cursor` and `catchup_cursor`. If no
checkpoint exists, they initialize from config or latest block. This is the
universal restart mechanism. Maintenance advances cursors conservatively for
state that successfully reached `MessageBuffer`, but the checkpoint is not a
per-block processing acknowledgement: post-`getLogs` failures and direct
catchup completion can move scan coverage past work that never reached the
buffer. See `indexing-gaps-retries-and-checkpoint-safety.md`.

**What the three cursors mean.** All are **inclusive "next block to scan"**
boundaries, not "last block scanned" — which is why a restored boundary block is
re-scanned. Together, `catchup_min_cursor` and `catchup_max_cursor` delimit the
**interval that has not been scanned yet**:

```text
   started_at_block                                                     chain tip
        │                                                                   │
        ▼                                                                   ▼
   ─────┬───────────────┬──────────────────────┬─────────────────────┬──────────
        │   scanned     │     NOT scanned      │       scanned       │ not yet
        │  (upward      │   (catchup work      │  (downward catchup  │ reached
        │   catchup)    │     remaining)       │   + realtime)       │
   ─────┴───────────────┴──────────────────────┴─────────────────────┴──────────
                        ▲                      ▲                     ▲
              catchup_min_cursor       catchup_max_cursor      realtime_cursor
              inclusive, only rises    inclusive, only falls   inclusive, only rises
```

| Cursor | Meaning | Direction | Conflict rule |
|---|---|---|---|
| `catchup_min_cursor` | lowest block not yet scanned | only rises | `GREATEST` (see caveat) |
| `catchup_max_cursor` | highest block not yet scanned | only falls | `LEAST` |
| `realtime_cursor` | next block the forward scan will request | only rises | `GREATEST` |
| `finality_cursor` | intended confirmed-block boundary | — | unused |

Consequences worth internalizing:

- **Catchup completion is "the two catchup cursors met"**, i.e. the unscanned
  interval is empty (`catchup_max_cursor < catchup_min_cursor`). It is not "the
  frontier reached `started_at_block`" — that formulation only happens to work
  while `catchup_min_cursor` sits at `started_at_block`.
- **The model allows catchup from both ends at once** — downward from realtime
  and upward from `started_at_block`. Nothing in the schema or the cursor
  algebra prevents it. Both current indexers walk **downward only**, so a reader
  should treat one-directional catchup as an implementation property, not as the
  contract.
- `realtime_cursor` is monotonically the highest block ever known for the pair,
  so it doubles as a usable upper anchor for progress calculations.

**Current caveat — `catchup_min_cursor` is written but never used.** Both writers
hard-code it to `0` (`message_buffer/persistence.rs:394`,
`database.rs:2512`) and no reader consumes it, so today the lower bound of the
unscanned interval is implicit in `started_at_block` from config rather than
stored. The column is therefore *dormant, not dead*: the semantics above are the
schema's design, and the hard-coded `0` is the gap between design and
implementation. `finality_cursor` is likewise written as `0` and read nowhere,
but that one has no implementation behind it at all — there is no
confirmation-depth or reorg logic (see
`indexing-gaps-retries-and-checkpoint-safety.md`).

#### 3. Buffer mutation via `alter()`

`buffer.alter(key, chain_id, block_number, mutator)` is the sole entry point
from any indexer into the shared pipeline. It:

1. Gets or creates the entry (checking hot tier first, then restoring from
   `pending_messages` cold tier on miss, or creating a new default)
2. Applies the protocol-specific `mutator` closure to the inner `T`
3. Records `(chain_id, block_number)` in the entry's `touched_blocks` for
   cursor tracking
4. Increments the entry `version` (marks it dirty for next maintenance)

Cold-tier restore resets `hot_since` to `Utc::now()` so the entry gets a full
TTL in memory.

#### 4. `Consolidate` trait — the protocol boundary

Each protocol defines a type `T: Consolidate` with:

```rust
fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>>
```

Three logical outcomes:

- `Ok(None)` — not yet consolidatable (missing required events). Buffer keeps
  the entry.
- `Ok(Some(ConsolidatedMessage { is_final: false, .. }))` — partial: can
  produce canonical rows but the message is not yet final. Flushed to DB but
  kept in buffer for further updates.
- `Ok(Some(ConsolidatedMessage { is_final: true, .. }))` — complete: flushed
  to DB and evicted from buffer.

`ConsolidatedMessage` contains a `crosschain_messages::ActiveModel`, a
`Vec<crosschain_transfers::ActiveModel>`, and `is_final`. This is the universal
output shape — every protocol must produce it regardless of internal event
model.

#### 5. `Key` contract

`Key` is `(message_id: i64, bridge_id: i16)` — compact for FK efficiency
across the schema.

- If the protocol's native message ID fits into `i64`, the indexer can use it
  directly as `message_id` and leave `crosschain_messages.native_id` empty.
- If the native ID is larger or differently typed (e.g., 32-byte hash), the
  indexer derives a compact `i64` and stores the original in the optional
  `native_id` field.
- Uniqueness per bridge is the indexer's responsibility in either case.

#### 6. Maintenance loop

A background task runs `buffer.run()` on a configurable
`maintenance_interval` (default 500ms). Each cycle:

**Planning phase** (`plan_maintenance`):

Every hot-tier entry is classified based on:

- **Dirty check**: `version > last_flushed_version` — skip unchanged entries
- **Consolidation**: call `T::consolidate()` → `NotReady`, `Partial`, or
  `Complete`
- **Staleness**: `age >= hot_ttl` — stale entries are offloaded regardless of
  consolidation result

Classification outcomes:

| Dirty? | Consolidation | Stale? | Action |
|--------|---------------|--------|--------|
| No | — | No | `Unchanged` — skip consolidation/flush, keep as hot cursor barrier |
| No | — | Yes | `Unchanged` + stale — offload to cold tier |
| Yes | `None` | No | `NotReady` — stays in hot tier (hot cursor barrier) |
| Yes | `None` | Yes | `NotReady` + stale — offload to cold tier |
| Yes | `Some(final=false)` | No | `Partial` — flush to DB, mark flushed, keep in hot |
| Yes | `Some(final=false)` | Yes | `Partial` — flush to DB, offload to cold tier |
| Yes | `Some(final=true)` | — | `Complete` — flush to DB, evict from hot |

**Commit phase** (`commit_maintenance`) — single DB transaction:

1. `offload_stale_to_pending(tx, stale_entries)` — serialize buffer entries as
   JSON into `pending_messages` (upsert)
2. `flush_to_final_storage(tx, consolidated_entries)` — upsert into
   `crosschain_messages` and `crosschain_transfers`
3. `stats.apply_stats_for_flushed_batch(tx, flushed)` — inline stats
   projection for **every flushed entry, final and `Partial`** (renamed from
   `apply_stats_for_finalized_batch` and widened past finalized-only; see
   `.memory-bank/research/stats-projection.md` and
   `.memory-bank/research/stats-subsystem.md` for why: token/asset identity
   maintenance must see every flushed canonical key, while counting itself
   stays gated on eligibility inside the projection functions)
4. `remove_finalized_from_pending(tx, finalized_keys)` — delete finalized
   entries from `pending_messages` (still keyed on `is_final`, unaffected by
   the widened stats trigger)
5. `fetch_cursors` + `calculate_updates` + `upsert_cursors` — derive and
   persist new checkpoint positions

**Post-commit phase** (outside the transaction):

6. `kickoff_token_enrichment_for_flushed(flushed)` — extract distinct
   `(chain_id, token_address)` pairs from every flushed transfer (final and
   `Partial`, renamed from `kickoff_token_enrichment_for_finalized`) and
   trigger async token metadata fetch
7. `mark_flushed_versions(keys_to_mark_flushed)` — update
   `last_flushed_version` for partial entries so they won't be re-flushed until
   mutated again
8. `remove_from_hot_if_unchanged(hot_evictions)` — CAS removal: only evict if
   entry version hasn't changed since planning (prevents racing with concurrent
   `alter()` calls)

#### 7. Cursor derivation

Cursor tracking determines how far `indexer_checkpoints` can safely advance.

**Block classification during planning:**

- Entries leaving the hot tier (stale or finalized) contribute their
  `touched_blocks` as **cold** — finalized state is flushed, while stale
  incomplete state is first persisted in `pending_messages`, so it can be
  restored after restart.
- Entries remaining in the hot tier contribute their `touched_blocks` as
  **hot** — these blocks contain pending work and act as barriers.

**Cursor calculation (`CursorBlocksBuilder`):**

- For existing checkpoints: `BlockSets::extend_cursor()` walks cold blocks
  from the current position, bridging gaps, stopping at hot barriers. Backward
  cursor stops at `hot_block + 1`, forward cursor stops at `hot_block - 1`.
- For new checkpoints (bootstrap): `BlockSets::bootstrap_cursor()` finds the
  longest contiguous range of cold blocks not interrupted by hot blocks.

**Persistence invariants:**

- `catchup_max_cursor` uses `LEAST(existing, new)` — can only decrease
  (backward scanning)
- `realtime_cursor` uses `GREATEST(existing, new)` — can only increase
  (forward scanning)
- `catchup_min_cursor` is written as constant `0` and never read — see the cursor
  semantics table in step 2 for what it is *meant* to hold
- This preserves cursor monotonicity but does not prove full block processing.
  Gap bridging is safe only when missing block numbers truly contain no failed
  work. Blocks that fail after a successful `eth_getLogs` are absent from both
  hot and cold sets and can therefore be crossed. See
  `indexing-gaps-retries-and-checkpoint-safety.md`.

#### 8. `CrosschainIndexer` trait

Every indexer implements `start()` and `stop()` for lifecycle management.
Concrete cleanup strategies (drop guards, abort patterns) are implementation
choices, not specified by the trait.

---

### Layer 2: Avalanche Reference Realization

#### 1. LogStream configuration

One `LogStream` per configured chain, filtered by the chain's Teleporter
contract address and `ITeleporterMessengerEvents` signatures. All per-chain
streams are merged via `SelectAll` for interleaved processing.

#### 2. Checkpoint initialization

If no checkpoint exists for a `(bridge_id, chain_id)` pair, `realtime_cursor`
is set to `provider.get_block_number()` and `catchup_cursor` to `latest - 1`.
Otherwise restored from `indexer_checkpoints` via the generic mechanism.

#### 3. Transaction-grouped processing

Logs are batched by block by LogStream, then grouped by transaction hash. For
each transaction, the indexer:

1. Fetches the full receipt (to access non-Teleporter ICTT logs)
2. Fetches the block (for `block_timestamp`)
3. Dispatches each Teleporter log to a typed handler

Receipt fetching is parallelized (`buffer_unordered(25)`).

#### 4. Blockchain ID resolution

Teleporter events identify peer chains by 32-byte Avalanche `blockchain_id`.
`BlockchainIdResolver` translates these to numeric EVM `chain_id` before bridge
filtering. Resolution order: in-memory cache → DB → Avalanche Data API.

See `avalanche-blockchain-id-resolution.md` for full details.

#### 5. Bridge filtering

`should_process_message(source, dest, chain_ids, process_unknown_chains,
home_chain)` is applied after blockchain ID resolution, before buffer mutation.
Two-stage filter:

- **Chain config filter**: both configured → pass; one configured, one
  unknown → pass only if `process_unknown_chains = true`; both unknown → reject
- **Home chain filter**: if `home_chain` is set, at least one endpoint must
  equal it

#### 6. Event handlers and buffer mutation

Four Teleporter events are handled, each calling `buffer.alter()`:

**`SendCrossChainMessage`** (source-side):

- Resolves `destinationBlockchainID` → EVM chain ID
- Applies bridge filter
- Parses sender-side ICTT logs from the same receipt (`TokensSent`,
  `TokensAndCallSent`, `TokensRouted`, `TokensAndCallRouted`), correlated via
  `teleporterMessageID`
- Sets `msg.send` and `msg.transfer` (source side)

**`ReceiveCrossChainMessage`** (destination-side):

- Resolves `sourceBlockchainID` → EVM chain ID
- Applies bridge filter
- Sets `msg.receive` and `msg.source_chain_is_unknown` if source chain is not
  in configured chain set
- Detects execution outcomes in the same tx but intentionally does NOT persist
  them (`_maybe_execution` pattern)

**`MessageExecuted`** (destination-side, success):

- Resolves `sourceBlockchainID` → EVM chain ID
- Applies bridge filter
- Sets `msg.execution = Succeeded`
- Parses receiver-side ICTT logs (`TokensWithdrawn`, `CallSucceeded`,
  `CallFailed`) with one-outcome-per-receipt invariant enforced
- Updates `msg.transfer` with destination side

**`MessageExecutionFailed`** (destination-side, failure):

- Resolves `sourceBlockchainID` → EVM chain ID
- Applies bridge filter
- Sets `msg.execution = Failed` **only if not already `Succeeded`**

#### 7. `Message` type — incremental assembly

The `Message` domain type accumulates events from both source and destination
chains over time. Events may arrive in any order across multiple maintenance
cycles:

- `send: Option<AnnotatedEvent<SendCrossChainMessage>>`
- `receive: Option<AnnotatedEvent<ReceiveCrossChainMessage>>`
- `execution: Option<MessageExecutionOutcome>` — `Succeeded` or `Failed`
- `transfer: Option<TokenTransfer>` — ICTT transfer (optional)
- `source_chain_is_unknown: bool` — enables fallback consolidation

#### 8. Consolidation rules (`Consolidate for Message`)

**Source data extraction** (determines if consolidation can proceed):

- If `send` is present → use it (normal path, has all source-side data)
- If `send` is absent and `source_chain_is_unknown = true` → fall back to
  `receive` or `execution` event (degraded path with partial data)
- If `send` is absent and `source_chain_is_unknown = false` → not ready
  (`None`) — wait for send event from the configured source chain

**Destination chain ID**: collected from all present events and verified for
consistency across send/receive/execution.

**Status determination**:

- `Completed` — execution succeeded (`MessageExecuted` received)
- `Failed` — execution failed (`MessageExecutionFailed` received)
- `Initiated` — no execution outcome yet

**Finality** (`is_final`):

- Execution must have succeeded, **AND**
- ICTT transfer must be complete
- Failed messages are **never final** — they can be retried via
  `retryMessageExecution()`
- Messages without ICTT transfers: `is_final = execution_succeeded`
- "ICTT transfer complete" is **not** simply "both source and destination
  sides present" — the destination side may legitimately never exist. The
  ICM payload (`TeleporterMessage.message`, decoded by `ictt_payload.rs`) is
  classified into a credit expectation: `SINGLE_HOP_SEND` / `SINGLE_HOP_CALL`
  ⇒ a destination credit (`TokensWithdrawn` / `CallSucceeded` / `CallFailed`)
  is expected; `REGISTER_REMOTE` / `MULTI_HOP_SEND` / `MULTI_HOP_CALL` ⇒ none
  ever will be (a `MULTI_HOP_*` arriving at a home re-sends under a *new*
  message id instead of crediting a recipient). A transfer with the source
  side present, no destination side, and "no credit expected" now completes
  — this is what closes the finality bug below. Classification runs
  regardless of whether `send` or the `(None, true)` fallback path supplied
  the payload, so it also fixes a fully indexed multi-hop first leg.

**Transfer building**: the `send`-driven builder (`build_transfer`) still
requires both `send` and `transfer`. A second path,
`try_reconstruct_transfer` / `build_reconstructed_transfer`, builds a
transfer from the ICM payload alone when `send` is absent, the source chain
is unknown, the payload classifies as `SINGLE_HOP_SEND` / `SINGLE_HOP_CALL`,
and a receiver-side ICTT effect corroborates it. It never fires when `send`
is present or the source chain is configured — that would race the real
`send` event. A per-bridge kill switch
(`bridges.json.reconstruct_incoming_ictt_transfers`, default `true`) is
applied at ingestion, not here: when disabled, a source-side-less receiver
ICTT arm is simply never recorded for a source-unknown message, so this
builder has nothing to work with.

**The finality bug this closes** (historical): before this change,
"transfer complete" required both sides present, so an incoming ICTT
message from an unindexed source chain — which can never produce a `send`
event and therefore never gets a destination-paired *source* side — stayed
`Partial` forever, was never stats-projected, and accumulated permanently in
`pending_messages`. The same bug fired on a fully indexed multi-hop first
leg, whose home routes onward instead of crediting a recipient. Both
triggers are fixed by classifying "no credit expected" as complete, not by
requiring a reconstructed row — if the payload has not arrived yet, the
message finalizes with no transfer, is evicted, and a later flush (once the
payload does arrive) re-writes the same `(message_id, bridge_id, 0)` key
safely, because both on-conflict builders omit `stats_processed` from the
update list.

#### 9. Key derivation

First 8 bytes of Teleporter `messageID` as big-endian `i64`. Original 32-byte
ID stored in `crosschain_messages.native_id`.

#### 10. `IndexerCleanupGuard`

Drop guard pattern used by `AvalancheIndexer`. On drop: resets `is_running`,
aborts the buffer maintenance task, clears the indexing handle, and sets state
to `Idle` (or preserves `Failed` if already set). This is an implementation
pattern, not a generic contract, but may be reused by future indexers.

## Invariants

### Generic

- `buffer.alter()` is the normal path from protocol event handlers to the
  shared message pipeline
- Protocol handlers do not write `pending_messages` or `indexer_checkpoints`
  directly, but `LogStream` directly calls
  `InterchainDatabase::mark_catchup_complete()`
- Maintenance is the sole writer to `crosschain_messages`,
  `crosschain_transfers`, and `pending_messages`; it is not the sole writer to
  `indexer_checkpoints`
- The maintenance transaction is atomic: all five steps commit or roll back
  together
- Stats projection runs inside the maintenance transaction, not after
- Stats projection and token enrichment are triggered for **every flushed
  entry each cycle, final and `Partial` alike** — not only finalized ones.
  `is_final` still gates pending-tier cleanup, hot-tier eviction, and
  finalized-batch metrics; it does not gate whether an entry reaches these two
  hooks. Whether a flushed row is actually *counted* by stats is a separate
  eligibility decision made inside the projection functions themselves — see
  `.memory-bank/research/stats-projection.md`
- Token enrichment runs outside the transaction (post-commit)
- Cursors can only advance monotonically in their scanning direction
- Hot-tier eviction uses CAS: concurrent mutations between planning and
  eviction prevent entry removal
- `BufferItem.version` monotonically increases; `last_flushed_version` tracks
  the last successfully flushed version to avoid redundant upserts

### Avalanche-specific

- Blockchain ID resolution happens before bridge filtering
- Bridge filtering happens before buffer mutation — filtered events never
  enter the buffer
- `ReceiveCrossChainMessage` handler does not persist execution outcomes even
  when detected in the same receipt
- `MessageExecutionFailed` does not overwrite a previously observed `Succeeded`
  outcome
- Receiver-side ICTT effects are parsed only during `MessageExecuted` handling
- One sender-side ICTT transfer per Teleporter message per receipt is enforced
- One receiver-side ICTT outcome per receipt is enforced
- Destination chain ID consistency across all present events is verified during
  consolidation
- Incoming-ICTT-transfer reconstruction only fires in `consolidate()`'s
  `(None, true)` branch (`send` absent, source chain unknown); it never races
  a real `send` event
- The per-bridge `reconstruct_incoming_ictt_transfers` kill switch never drops
  a receiver-side ICTT arm for a message whose source chain **is** configured
  — the ingestion gate keys on `source_is_unknown`, not on "this arm has no
  source side", because `parse_sender_ictt_log` legitimately produces a
  source-side-less arm for a configured-source message whose destination logs
  arrived before its `send` log
- `Message`'s buffered shape is unaffected by the kill switch: it is applied
  where the receiver-side ICTT arm is recorded (`avalanche/mod.rs`), not as a
  field on `Message`

## Failure Modes / Observability

### Generic

- Buffer maintenance failure is logged and increments
  `BUFFER_MAINTENANCE_ERRORS_TOTAL`; the loop continues on next tick
- Cold-tier restore failure (DB or deserialization) propagates from
  `buffer.alter()` to the indexer's per-log error handling
- Cursor advancement inside maintenance is transactional: if maintenance fails
  mid-transaction, its cursor update rolls back. This guarantee only covers
  work represented in that maintenance plan; it does not cover post-`getLogs`
  processing failures or the separate catchup-completion writer. See
  `indexing-gaps-retries-and-checkpoint-safety.md`
- Per-bridge metrics: `BUFFER_HOT_ENTRIES`, `BUFFER_MAINTENANCE_ENTRIES` (by
  state), `BUFFER_EVICTED_ENTRIES` (by reason), `BUFFER_MESSAGES_FINALIZED_TOTAL`,
  `BUFFER_TRANSFERS_FINALIZED_TOTAL`, `BUFFER_CURSOR` (by direction),
  `BUFFER_MAINTENANCE_DURATION`

### Avalanche-specific

- Log batch processing errors are logged per-batch; the stream continues
- Blockchain ID resolution failures propagate to the log handler (message
  skipped)
- Filtered messages produce trace-level logs with full context (message ID,
  chains, filter reason)
- Receipt/block collection is all-or-nothing: one failure aborts the entire
  fetched batch before event dispatch, and the outer loop logs and continues
  without requeueing that range

## Edge Cases / Gotchas

### Generic

- Partial (non-final) entries are flushed to DB but kept in buffer — they
  produce upserts on every maintenance cycle where they are dirty, which is
  correct but can generate write amplification for long-lived partial entries
- `BufferItemVersion` is `u16` — overflows after 65535 mutations to the same
  entry. Currently caught by `checked_add` returning an error.
- Stale entries are offloaded AND their consolidation result (if any) is
  flushed in the same cycle. A stale partial entry gets both an upsert to
  canonical tables and a cold-tier write.
- Hot-tier CAS eviction can be skipped if the entry was mutated between
  planning and post-commit — the entry stays hot with a fresh TTL
- `pending_messages` payload is the full serialized `BufferItem<T>` including
  `touched_blocks` — cold-tier entries retain cursor context across hot/cold
  cycles

### Avalanche-specific

- `_maybe_execution` in the receive handler is detected but unused — either a
  planned future wire-up or dead code
- Message key uses first 8 bytes of a 32-byte hash — collision is
  theoretically possible but practically unlikely for current Teleporter
  behavior
- Routed ICTT variants (`TokensRouted`, `TokensAndCallRouted`) use
  `Address::ZERO` as sender address since the routing contract is the caller,
  not the original sender

## Change Triggers

Update this note when:

- the `Consolidate` trait signature or `ConsolidatedMessage` shape changes
- `buffer.alter()` API or cold-tier restore behavior changes
- maintenance transaction step ordering changes
- cursor derivation logic (cold/hot classification, extend/bootstrap) changes
- `catchup_min_cursor` or `finality_cursor` gains a real runtime writer or reader
  (both are currently written as constant `0`; the cursor semantics table in
  Layer 1 step 2 must then be updated to describe behaviour rather than intent)
- catchup gains an upward scanning direction, making the two-ended unscanned
  interval real rather than latent
- new post-commit hooks are added to the maintenance pipeline
- `LogStream` API or bidirectional scanning model changes
- `CrosschainIndexer` trait contract changes
- Avalanche event handler set changes (new events, changed dispatch)
- Avalanche consolidation rules change (finality, status, source data)
- Avalanche ICTT parsing logic changes
- a new indexer is implemented (add a pointer to its separate research note)

## Open Questions

- **Execution overwrite asymmetry:** `MessageExecutionFailed` won't overwrite
  `Succeeded`, but `MessageExecuted` unconditionally overwrites any prior
  outcome. Whether this is intentional or a gap is unknown.
- **Receive handler unused execution detection:** The `_maybe_execution`
  pattern is present but unused — is this a planned future wire-up or dead code
  to remove?
- **`BufferItemVersion` overflow:** `u16` limits a single entry to 65535
  mutations. Is this sufficient for all realistic message lifecycles, or should
  it be widened?
