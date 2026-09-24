# ADR-014: The First-Seen Destination Execution Is Canonical; Later Ones Are Recorded, Not Merged

**Date:** 2026-09-22

**Authors:** @EvgenKor

## Context

An xDai destination event (`AffirmationCompleted`, `RelayedMessage`) carries a
single `bytes32` field whose meaning changed with the May 2025 upgrade but whose
type and position did not. The oracle writes either a source **nonce** or a
source **transaction hash** into it, and `MessageIdentity::destination` tells
them apart only by magnitude (`<= u64::MAX` is a nonce). On Chiado this produced
a period in which one modern nonce-bearing transfer was executed **twice**: once
keyed by its nonce and once keyed by its source transaction hash, in different
transactions and sometimes on opposite sides of a grammar upgrade. Three of the
four testnet transfers in the reopened window are in this state; the fourth has
a single hash-keyed completion and is perfectly ordinary.

Two facts forced a decision rather than a fix:

- **A hash-keyed completion is not an error.** It is the only representation
  pre-nonce transfers ever had, and the indexer already resolves those through a
  source-receipt lookup. Treating "identity is a hash" as the anomaly predicate
  would break the shipped pre-nonce destination path and would still miss a
  second execution that arrived nonce-keyed.
- **The payout really happened twice.** Both executions are on chain. Collapsing
  them into one row silently discards evidence; keeping two canonical rows for
  one transfer breaks the one-message-per-transfer contract the API and stats
  depend on.

Ordering makes this worse: catch-up scans blocks **backwards**, so whichever
execution is seen first is not the chronologically first one, and a
reindex can legitimately pick the other.

## Decision

Canonical identity and observed identity are separated, and multiplicity —
not identity kind — is the anomaly predicate.

1. **Canonical identity comes from source evidence.** A hash-keyed completion
   still fetches its source receipt. If that receipt holds a modern
   nonce-bearing source event whose recipient matches the payout, the message's
   canonical identity is that **nonce**; the raw hash survives only as
   observation provenance. A genuine two-argument event or a supported no-event
   deposit keeps the raw hash as canonical identity, exactly as before. A
   nonce-keyed completion fetches nothing.
2. **One source transaction carries one transfer.** More than one modern source
   event in a receipt, or a single one whose recipient differs from the payout
   recipient, is a **hard error** that reaches `indexer_failures`. This is a
   detector, not a safety net: the model says it cannot happen, and we want to
   be told if it does. Value is never a gate and never a tie-break.
3. **First-seen wins, in processing order.** The first destination execution
   observed for a canonical key owns `crosschain_messages` and
   `crosschain_transfers`. A later, different execution can never replace
   `dst_tx_hash`, recipient, destination amount, or the canonical
   `last_update_timestamp`.
4. **Later executions are recorded twice, for two audiences.** Internally, one
   row per distinct later execution in the existing `amb_message_anomalies`
   table (`event_kind = 'destination_execution'`, canonical key in `buffer_key`,
   canonical transaction in `conflict_tx_hash`). Publicly, a sparse
   `protocol_metadata.multiple_executions` namespace carrying the later
   executions themselves — transaction hash and timestamp, nothing else —
   rendered into the message's `extra`. The two representations must name the
   same transactions.
5. **The decision is made against storage, inside the maintenance
   transaction.** `consolidate` cannot see the database, so protocols report
   observations through a defaulted `Consolidate::destination_executions`
   channel and `persistence::reconcile_destination_executions` compares them to
   the stored row. Anomaly insert, metadata patch, pending cleanup and cursor
   advancement commit together or not at all.
6. **Nothing is promoted without somewhere to point.** The anomaly table has no
   foreign key, so a later execution is recorded only when the canonical row is
   already stored or is being written by the same transaction. Until then the
   observation stays in the hot buffer / `pending_messages`; a completion alone
   never permits final eviction.

## Alternatives Considered

### Alternative 1: Treat a hash-keyed completion as the anomaly

**Pros:**

- Trivial predicate, no source receipt needed to classify.

**Cons:**

- Wrong by construction: it flags the ordinary pre-nonce path as anomalous and
  misses a second execution that arrives nonce-keyed.
- Would have to be undone the moment pre-nonce source windows are opened.

### Alternative 2: Keep both executions as two canonical messages

**Pros:**

- No new concepts; each execution is just a message.

**Cons:**

- Two rows for one transfer double every statistic derived from them and give
  the API two answers to one question.
- The source transaction genuinely produced one transfer; the duplication is a
  destination-side artefact and belongs in destination-side evidence.

### Alternative 3: Pick the canonical execution chronologically

**Pros:**

- Stable across reindexing; "the first payout" is a meaningful sentence.

**Cons:**

- Requires either holding every key open until catch-up passes it, or rewriting
  a stored canonical row later — the second is exactly what criterion "a later
  execution cannot replace `dst_tx_hash`" forbids, and the first breaks
  eviction.
- Buys little: the invariant that matters downstream is "one canonical row plus
  a complete list of the others", and that holds under either choice.

## Consequences

### Positive

- The pre-nonce destination path shipped in `xdai-legacy-destination-events`
  keeps working untouched, and the future pre-nonce **source** handler lands on
  the same raw-hash key that its confirmations and completions already use — no
  new storage identity, no second destination pipeline.
- Double payouts stop being invisible. They are queryable internally and visible
  in the public API without exposing the internal anomaly columns.
- Protocols that do not participate pay nothing: the trait method is defaulted
  to empty and both reconciliation functions return before their first query, so
  AMB and Avalanche execute the same statements as before.

### Negative

- Which execution is canonical depends on indexer processing order, so a full
  reindex can move a transaction from `dst_tx_hash` into
  `multiple_executions` and back. This is accepted and documented rather than
  defended against.
- Hash-keyed confirmations of a completion that was re-keyed to a nonce stay on
  the raw-hash key and are never consolidated — four such rows on the testnet.
  Correlating them would need a source receipt per confirmation, and
  confirmations outnumber completions by the validator count.
- The anomaly dedupe key leads with `(bridge_id, buffer_key)` while the table's
  only index is `(bridge_id, native_id)`, so the dedupe read does not use it.
  Accepted at the current table size.

### Neutral

- `amb_message_anomalies` is reused unchanged for a non-AMB protocol; the table
  is a general "displaced evidence" sink, and its name is now narrower than its
  role.
- The anomaly's `native_id` holds the raw observed 32 bytes, which for a
  nonce-keyed observation differs from the canonical `chain‖nonce` blob in
  `crosschain_messages.native_id`. Deliberate: it records what was seen.

## References

- ADR-010 — the `protocol_metadata` namespace mechanism this reuses.
- ADR-013 — xDai deployment constants and grammar windows, including the Chiado
  epoch floor this task lowered to `15_562_365`.
- `.memory-bank/research/xdai-bridge-sepolia-chiado-upgrade-history.md` — the
  upgrade timeline and the on-chain evidence for the duplicated executions.
- `interchain-indexer-logic/src/indexer/xdai/events.rs` —
  `decode_source_evidence`, canonical identity selection in the completion
  handlers.
- `interchain-indexer-logic/src/message_buffer/persistence.rs` —
  `reconcile_destination_executions`,
  `apply_destination_execution_reconciliation`.
