# ADR-021: Message Status Is A Shared Base Set With Non-Terminal Extensions

**Date:** 2026-11-25

**Authors:** @EvgenKor

**Status:** Accepted

## Context

- `crosschain_messages.status` started as `initiated`, `completed`,
  `failed`; AMB added `ready_to_claim`. OP interop needs more in-flight
  values (a relay not yet confirmed, a reorged message).
- Terminality was implicit, in three unconnected places: an SQL literal in
  the upsert guard (`crosschain_messages_on_conflict`), each indexer's
  `is_final`, and the status branch of the stats gate. A new value was
  non-terminal for the guard by default, and nothing forced a decision.
- `failed` is not always final in protocol terms: Avalanche retries it into
  `completed` (`retryMessageExecution`).
- Two paths can undo a status without checking terminality:
  `replace_existing` deletes the canonical row, and a retraction cannot be
  expressed as an upsert, because destination columns are only ever filled.
- A deep reorg can, in principle, remove a destination leg after its
  message was stored as `completed` (the PRD: relayed (final) is "terminal,
  modulo deep reorg").

## Decision

1. **Base set.** `initiated` (the default, non-terminal), `completed` and
   `failed` (terminal) mean the same on every bridge.
2. **Terminal means "never again non-terminal".** A move between the two
   terminal values is allowed only for a protocol-defined retry (Avalanche
   `failed → completed`). The upsert guard enforces the first half; the
   second is each indexer's responsibility (a stale `completed → failed`
   flush is not stopped).
3. **Extensions are non-terminal.** Every value a protocol adds refines the
   not-yet-terminal state. A terminal outcome is stored as `completed` or
   `failed`, with the protocol's reason in `protocol_metadata`, rendered into
   `extra`. Records that are not messages carry no status (OP raw
   executions, ADR-020).
4. **`is_final` implies a terminal status**, not the converse. Maintenance
   (`classify_item`) evicts an entry as finalized only when it is final
   **and** terminal. (xDai's reconciliation channels, ADR-014 / ADR-015,
   evict by a database-side resolution, independent of `is_final`, and never
   write the status.) A final entry with a non-terminal status is kept as
   partial and logged at `error`; it panics only under `cfg(test)`, so the
   crate's tests catch it while debug and release builds of the service keep
   running. It is never an `Err`.
5. **One definition.** `MessageStatus::is_terminal()`, in the entity crate's
   `manual/` module, is an exhaustive `match` and the only definition of
   terminality. The guard SQL is rendered from it. The terminal set is never
   written as a literal.
6. **Countability is separate.** The status branch of
   `finalized_message_stats_condition` admits only terminal values; which
   ones count per bridge is a stats decision (today `completed` on every
   bridge, `failed` on AMB). The observability-horizon branch (ADR-004) is
   status-agnostic by design and is the stated exception.
7. **Status describes the protocol lifecycle only** (ADR-004, decision 4.3).
8. **Adding a value.** A value is added in the change that first writes it,
   because PostgreSQL cannot drop an enum value. The proto value gets the
   `MESSAGE_STATUS_` prefix and the next number. Frontend handling ships
   before the backend writes the value. The checklist is in
   `rules/database.md`, "Message Status Values".
9. **Deleting or rewriting a canonical row.** A new path that does it must
   refuse a row with a terminal status or with `stats_processed > 0`. AMB's
   `replace_existing` predates this rule and stays as it is.
   *Amended 2026-11-26:* its first user is the OP guarded overwrite
   (`crosschain_messages_overwrite_unsettled`, ADR-022).
10. **Terminal is relative to the indexer's confirmation boundary.** An
    indexer stores a terminal status only for data it treats as final (for
    OP interop: every leg at or below the confirmation tag, ADR-018). A
    reorg below that boundary is outside this model: the guard blocks
    `completed → initiated`, and so does the rest of the pipeline
    (nothing detects such a reorg: a retry or rescan re-reads the new
    canonical blocks, where a vanished relay simply yields no log; the
    upsert cannot clear destination columns; stats have no decrement path;
    checkpoints only move forward).
    With `finalized` it needs an L1 finality violation; with `safe` an L1
    reorg can roll the safe head back, so the risk is not zero. Handling it
    would be a separate repair path (detection below the tag, a dedicated
    statement that bypasses the upsert, stats subtraction), never a
    relaxation of the guard.

### Enforcement

- `interchain-indexer-entity/src/manual/mod.rs`:
  `test_message_status_is_terminal_only_for_completed_and_failed`.
- `message_buffer/persistence.rs`:
  `test_keep_terminal_sql_renders_the_former_literal`,
  `test_crosschain_messages_on_conflict_renders_the_guard_at_every_site`,
  `test_status_merge_keeps_terminal_against_non_terminal_and_never_clears_destination`.
- `message_buffer/maintenance.rs`:
  `test_is_complete_requires_is_final_and_a_terminal_status`,
  `test_classify_item_final_entry_with_non_terminal_status_panics_under_test`.
- `database.rs`:
  `test_stats_gate_counts_no_non_terminal_status_while_destination_is_indexed`.

The generic tests iterate `MessageStatus::iter()`, so a value added later is
covered with no edit.

## Alternatives Considered

### Alternative 1: Protocol-specific terminal values (`expired`, `relayed_final`)

**Cons:**
- Each needs a manual decision in the guard, the buffer and stats.
- The stats status branch silently drops it.

### Alternative 2: A terminal flag column or lookup table

**Cons:**
- A second source of truth next to the enum.

### Alternative 3: DB enforcement (a `CHECK` or trigger rejecting terminal → non-terminal)

**Cons:**
- A raised error inside the maintenance transaction stalls the bridge on
  every cycle.

### Alternative 4: A total ranking of statuses

**Cons:**
- Non-terminal moves go both ways (for example back to `initiated` after a
  retraction), and `failed → completed` must pass.

### Alternative 5: Derive the stats status branch from `is_terminal`

**Cons:**
- It would count `failed` on every bridge: a stats decision this ADR does
  not make.

### Alternative 6: `Err` or panic on a final, non-terminal entry

**Cons:**
- An `Err` stalls the bridge on every cycle; a panic crashes it.

## Consequences

### Positive

- A new value cannot compile without an explicit terminality decision.
- The guard SQL and eviction need no edit for a new value.
- On-chain expiry and relayed-final need no enum value.
- The generic tests cover future values.

### Negative

- A protocol's terminal flavour looks like a plain Completed / Failed to a
  client until it reads `extra`.
- Not closed:
  - the horizon branch counts any status when the destination is unindexed;
  - `replace_existing` is unguarded;
  - a stale `completed → failed` flush;
  - a terminal meaning stored under a non-terminal value, which only review
    catches;
  - frontend release ordering, which is discipline, not code;
  - a reorg below the confirmation boundary leaves a wrong `completed` row
    and its stats in place (decision 10).

### Neutral

- No behaviour change and no migration when introduced.

## References

- ADR-001, ADR-004, ADR-018, ADR-020
- `interchain-indexer-entity/src/manual/mod.rs` (`MessageStatus::is_terminal`)
- `message_buffer/persistence.rs` (`crosschain_messages_on_conflict`)
- `message_buffer/maintenance.rs` (`classify_item`)
- `stats/projection.rs` (`finalized_message_stats_condition`)
