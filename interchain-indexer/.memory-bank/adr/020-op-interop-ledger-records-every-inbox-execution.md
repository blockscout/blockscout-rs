# ADR-020: OP Interop Ledger Records Every `CrossL2Inbox` Execution And Is The Message Journal

**Date:** 2026-11-24

**Authors:** @EvgenKor

**Status:** Accepted

## Context

- Raw `CrossL2Inbox` messages must be indexed now and listed later. The PRD
  identity is `(Identifier, payload)` and the join key is `(chainId,
  blockNumber, logIndex)`. One source log can have N executions, with no
  replay protection.
- Every messenger message nests its own inbox level in `extra`.
- Positional pairing ("closest preceding") cannot tell a relay's own
  execution from a pull read in the same window. Once raw rows are stored,
  that would persist a wrong link.
- The lifecycle requirement: storing transitions starts now.

## Decision

1. **Ledger meaning.** `op_interop_events` holds observed interop logs; some
   are legs of a message. Every `ExecutingMessage` of any origin is one row.
   `message_id` / `message_hash` are nullable and set only on the delivering
   execution. The two hashes never mix: `msgHash` stays in `event_data`. The
   typed join key `src_chain_id` / `src_block_number` / `src_log_index`
   carries a partial index, and a value outside `BIGINT` is `NULL`. The
   table is named for what it holds, observed events, not message legs: it
   was `op_interop_legs` until 2026-11-25, renamed in place before any
   deployment (the in-memory `SourceLeg` / `DestinationLeg` keep their names;
   they are legs of a message).
2. **Ingestion write.** Raw executions are written per batch, unlinked,
   before the batch's buffer actions, through the same function and conflict
   policy as the flush (`upsert_op_interop_events`). A write failure attributes
   the executions' blocks to the failure ledger (ADR-005), and those blocks
   apply no buffer action in that attempt; the replay applies them whole. The
   link merges with `COALESCE` and is never cleared. *Amended 2026-11-26:*
   the ingestion write covers every leg, not only executions.
   `sent_message` / `relayed_message` rows are written linked, because a relay
   read before its source must have its row for a ledger-only sweep to find
   it. A write failure attributes the blocks of every row.
3. **Selection by content.**
   - A relay's candidates are the messenger-origin executions since the
     previous `RelayedMessage` whose `id.chainId` is its source.
   - `consolidate` links and nests the one that matches the held
     `SentMessage` (payload hash, chain, block, log index, timestamp), the
     lowest log index if several do.
   - With no match, nothing is nested or linked, and `identifier_mismatch`
     is counted.
4. **The ledger is the journal.**
   - Rows are never deleted.
   - From the optimistic overlay on, a row's state only moves forward
     (`provisional → confirmed | retracted`), and each move stamps its
     detection time once.
   - A revival or re-relay is a new row, because its block hash differs.
   - `crosschain_messages.status` is a cache of a pure fold over a message's
     rows.
   - Any new column needs a merge rule that is correct for both writers in
     either order.
   - *Amended 2026-11-26:* the state columns exist: `state`
     (`provisional | confirmed | retracted`, default `confirmed`),
     `confirmed_seen_at`, `retracted_seen_at` (UTC, each set once), a partial
     index on provisional rows, and the merge rule in
     `op_interop_events_on_conflict`. Until the overlay ships, every row is
     `confirmed`.
5. **No status, no listing.** Raw rows carry no `MessageStatus` and are not
   listed or counted: no list, search or stats path reads the ledger.

## Alternatives Considered

### Alternative 1: A dedicated raw table

**Cons:**
- A second sweep and conflict policy over the same data.
- The delivering execution is stored twice.

### Alternative 2: Hidden `crosschain_messages` rows

**Cons:**
- A predicate in every stats and list path, the external stats service
  included.
- Canonical rows that would have to be deleted.

### Alternative 3: A second bridge over the same chains

**Cons:**
- Rows land in per-bridge stats.
- `eth_getLogs` doubles.

### Alternative 4: Keep positional pairing

**Cons:**
- It persists wrong links.

### Alternative 5: An OP transition table, or a shared steps table

**Cons:**
- Second copies of the ledger's facts.

### Alternative 6: An array in `protocol_metadata`

**Cons:**
- It is replaced whole by the one-level merge on replay.

## Consequences

### Positive

- The raw layer is stored at no extra RPC cost.
- The nesting is correct by construction.
- One table, so later sweeps and retractions have one subject.

### Negative

- Two reads of the very log being relayed in one window are
  indistinguishable.
- A wrong link is never repaired by a replay.
- `process_batch` now writes to the database once per batch that has
  executions or legs.
- Candidates are stored in the buffer entry with no cap.
- From the overlay on, the ledger holds observations a reindex cannot
  rebuild.

### Neutral

- `extra.op_interop.executing_message` is unchanged.
- `created_at` of a delivering execution is its first ingestion.

## References

- ADR-005, ADR-008, ADR-018, ADR-019
- `research/op-interop-protocol-and-indexing-fit.md`
- `indexer/op_interop/`
