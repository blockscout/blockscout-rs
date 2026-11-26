# ADR-022: OP Interop Optimistic Overlay: Provisional Legs, Confirmation By Ledger Key, Retraction By Sweep

**Date:** 2026-11-26

**Authors:** @EvgenKor

**Status:** Accepted

## Context

- The confirmed-only OP interop indexer (ADR-018) reads every chain up to a
  confirmation tag (`finalized` by default) and lags `latest` by 20 to 30
  minutes.
- The product separates "relayed (unsafe source)", "relayed (final)" and
  "invalidated". Showing the first needs the data above the tag.
- Unsafe blocks can be replaced at the same height with a different hash; a
  relay in a block whose executing message is invalid is exactly such a block.
- The storage model is built for confirmed data only: canonical upserts are
  merge-only (ADR-021), stats are additive (no decrement path), cursors are
  forward-only, and the hot buffer tier is lost on a crash while
  `realtime_cursor` cannot pass a hot block (ADR-001).

The runtime (overlay loop, frontier, setting) is described below.

## Decision

1. **Ledger state.** A ledger row's state moves only
   `provisional → confirmed | retracted`, and each detection time is stamped
   once (UTC). Only the overlay writes `provisional`; only the confirmed
   stream writes `confirmed`; only the sweep (or the flush of an entry it
   retracted) writes `retracted`.
2. **Confirmation by key.** A leg's finality comes from its own chain's
   confirmed stream, matched by `(bridge_id, chain_id, block_hash,
   log_index)` at ingestion, never by block number: a replaced block keeps its
   height.
3. **Every leg is written when read** (ADR-020 amendment): the ledger write
   precedes the buffer action, and a buffered provisional leg always has a
   ledger row.
4. **Candidates and the fold.** A buffer entry holds a list of candidates per
   side, each with a state. The pure fold `lifecycle::status(sources,
   relays)` gives the stored status, which is a cache of the fold over the
   message's ledger rows:

   | sources | relays | status |
   | --- | --- | --- |
   | none | any | no row |
   | all retracted | any | `invalidated` |
   | an active one | none active | `initiated` |
   | active, confirmed | active, confirmed | `completed` |
   | active | active, one provisional | `pending_finality` |

   Ordering: confirmed first, then the highest block, then the lowest log
   index, then the lowest block hash. A retracted candidate is never active.
5. **Statuses.** `pending_finality` (relayed, an active leg not confirmed) and
   `invalidated` (every source retracted; kept visible, never deleted) are
   non-terminal (ADR-021). A relay-only retraction returns the message to
   `initiated` with `dst_tx_hash` and `last_update_timestamp` cleared.
   `extra.op_interop` legs carry `safety`; a retracted destination carries its
   `tx_hash`.
6. **Unsettled entries.** An entry holding any non-confirmed candidate is
   written with the guarded overwrite
   (`crosschain_messages_overwrite_unsettled`: `ON CONFLICT DO UPDATE SET col =
   EXCLUDED.col WHERE` the stored row is neither terminal nor counted) and,
   while partial, persisted to `pending_messages` in the same maintenance
   transaction. A canonical row never reflects a buffer state a crash could
   lose.
7. **No block recorded.** Overlay and sweep mutations record no block
   (`alter_unrecorded`, `alter_existing_durably`), so `realtime_cursor` never
   passes the confirmed head.
8. **The sweep.** It selects provisional ledger rows at or below a frontier,
   outside open failed ranges (ADR-005), and never creates an entry. Order is
   the durability rule:
   1. retract the legs in the buffer and persist the entries
      (`alter_existing_durably`);
   2. run one maintenance cycle, which flushes the canonical row and the
      entries' leg rows (provisional → retracted) in one transaction;
   3. only then mark the ledger rows `retracted`.

   A cold entry is restored only on demand and the sweep no longer selects a
   row once it is `retracted`, so without step 2 a stop after step 3 would leave
   the canonical row showing the retracted leg forever. A failure or stop before
   step 3 leaves the rows provisional; the next sweep repeats (the retract is a
   no-op, the entry is persisted again and flushed, then step 3).
9. **Stats.** The gate is unchanged (only terminal values count, ADR-021);
   `stats_chains` user counts exclude `invalidated` messages and their
   transfers (ADR-009).

### Runtime

Added with the overlay loop.

## Alternatives Considered

### Alternative 1: The sweep walks buffer entries

**Cons:**
- No height index over hot entries and JSON payloads; rebuilt after a restart.

### Alternative 2: The RPC tag head as the frontier

**Cons:**
- Retracts legs whose confirming batch is in flight; nodes disagree.

### Alternative 3: A retraction-only statement

**Cons:**
- Cannot express a revival or a re-relay.

### Alternative 4: A startup reconciliation instead of the write-through

**Cons:**
- Cannot rebuild a lost provisional-only entry.

### Alternative 5: Withholding `dst_tx_hash` until `completed`

**Cons:**
- Changes a generic field's meaning for one bridge.

### Alternative 6: A shared reorg mechanism for every indexer

**Cons:**
- The reverse stats projection is inexact.

## Consequences

### Positive

- Unsafe data is visible; stats, terminal rows and cursors keep their
  invariants.

### Negative

- The ledger holds observations a reindex cannot rebuild.
- `invalidated` and relay-retracted entries stay in `pending_messages` until
  revived.
- Retraction is detected up to one confirmation delay late.
- ADR-005's in-flight limitation (a failure lost before it reaches the failure
  ledger) can retract a canonical leg; a later confirmed read cannot revive it.
- The external stats service counts `dst_tx_hash IS NOT NULL` and every
  status (filled in with the overlay loop).

## References

- ADR-001, ADR-005, ADR-009, ADR-018, ADR-020, ADR-021
- `interchain-indexer-logic/src/indexer/op_interop/lifecycle.rs`, `sweep.rs`
- `message_buffer/persistence.rs`: `crosschain_messages_overwrite_unsettled`
