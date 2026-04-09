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

#### 2. Checkpoint restoration

On startup, indexers read `indexer_checkpoints` for their `(bridge_id,
chain_id)` pairs to determine `realtime_cursor` and `catchup_cursor`. If no
checkpoint exists, they initialize from config or latest block. This is the
universal restart mechanism — cursors are advanced conservatively during
maintenance so that no finalized work is repeated and no unfinished work is
skipped.

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
| No | — | — | `Unchanged` — skip entirely |
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
3. `stats.apply_stats_for_finalized_batch(tx, finalized)` — inline stats
   projection for the finalized subset
4. `remove_finalized_from_pending(tx, finalized_keys)` — delete finalized
   entries from `pending_messages`
5. `fetch_cursors` + `calculate_updates` + `upsert_cursors` — derive and
   persist new checkpoint positions

**Post-commit phase** (outside the transaction):

6. `kickoff_token_enrichment_for_finalized(finalized)` — extract distinct
   `(chain_id, token_address)` pairs from finalized transfers and trigger async
   token metadata fetch
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
  `touched_blocks` as **cold** — these blocks are fully processed and safe to
  advance past.
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
- This ensures cursors never skip unprocessed blocks on restart.

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
- ICTT transfer must be complete (both source and destination sides present),
  if applicable
- Failed messages are **never final** — they can be retried via
  `retryMessageExecution()`
- Messages without ICTT transfers: `is_final = execution_succeeded`

**Transfer building**: only built when both `send` and `transfer` are present.

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

- `buffer.alter()` is the sole path from indexers to the shared pipeline
- Indexers never interact with `pending_messages` or `indexer_checkpoints`
  directly
- Maintenance is the sole writer to `crosschain_messages`,
  `crosschain_transfers`, `pending_messages`, and `indexer_checkpoints`
- The maintenance transaction is atomic: all five steps commit or roll back
  together
- Stats projection runs inside the maintenance transaction, not after
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

## Failure Modes / Observability

### Generic

- Buffer maintenance failure is logged and increments
  `BUFFER_MAINTENANCE_ERRORS_TOTAL`; the loop continues on next tick
- Cold-tier restore failure (DB or deserialization) propagates from
  `buffer.alter()` to the indexer's per-log error handling
- Cursor advancement is conservative: if maintenance fails mid-transaction,
  cursors are not advanced, ensuring safe replay on restart
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
- Receipt/block fetch failures fail the entire batch for that transaction

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
