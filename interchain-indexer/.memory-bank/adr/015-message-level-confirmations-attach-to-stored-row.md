# ADR-015: Validator Confirmations Are Message-Level; Late Ones Attach To The Stored Row

**Date:** 2026-09-29

**Authors:** @EvgenKor

## Context

`amb_messages_confirmations` stores one row per `(message_id, bridge_id,
validator_address)`. For xDai, two mechanisms left in-range validator signatures
out of it permanently, parked in `pending_messages` entries that never
consolidate:

1. **Late confirmations after finalization.** Catch-up scans downward in
   `batch_size` windows. For Ethereum → Gnosis the fourth `SignedForAffirmation`
   and `AffirmationCompleted` share one Gnosis transaction, so a message whose
   source is already buffered finalizes, flushes and is evicted when that batch
   is processed. The next, older batch delivers signatures 1–3; `restore`
   misses, `alter` creates a fresh default entry under the same key, and it has
   no source, so it never consolidates. A full mainnet catch-up at
   `batch_size = 500` lost 621 signatures of 298 messages this way; every split
   between stored and orphaned signatures fell on a catch-up batch boundary.
   Realtime scans forward and does not trigger it; any late arrival after
   finalization (a failure-ledger retry of an old range, arbitrary in-batch
   dispatch order racing a maintenance cycle) does.
2. **A hash-keyed signature next to a hash-keyed completion.** When an oracle
   passes the source transaction hash instead of the nonce, the completion is
   re-keyed to the canonical nonce message through its source receipt
   (ADR-014), but the signature emitted in the same transaction was keyed on
   the raw hash. On Chiado this left nonce 2 `completed` with zero
   confirmations and orphaned the signatures of the three second executions.
   ADR-014 accepted this because "correlating them would need a source receipt
   per confirmation".

Independently of both, some hash-keyed signatures sign a disjoint on-chain
`hashMsg` bucket that never executes (mainnet: 347 signatures by validators
still on the old notation). They did not contribute to any execution.

## Decision

1. **Confirmations are message-level (policy P1).** A confirmation row of
   message K is a validator whose signature belongs to an executed signing
   bucket that resolved to K, whether that execution is canonical or recorded
   as a second execution in `amb_message_anomalies`. There is one row per
   validator, as the primary key already enforces. A bucket that never executed
   never counts.
2. **Detached-confirmation channel.** `Consolidate` gains a second defaulted
   method, `detached_confirmations(&self, &Key) -> Option<DetachedConfirmations>`
   (`None` by default). `plan_maintenance` asks every dirty `NotReady` entry for
   it. Inside the maintenance transaction, after the flush and before pending
   cleanup, `persistence::attach_detached_confirmations`:
   - returns before its first statement on empty input;
   - checks with a chunked `SELECT` which keys already have a
     `crosschain_messages` row — the FK is never used as a detector;
   - upserts those keys' confirmations `ON CONFLICT DO NOTHING`;
   - reports as resolved only keys whose entry held nothing but confirmations
     (xDai's `holds_only_confirmations`, an exhaustive destructuring of
     `Message`). Resolved keys join pending cleanup and are CAS-evicted **at
     most once per cycle** across both channels, because a CAS guards one entry
     instance, not re-creation (ABA).
   The fix does not depend on why a signature arrived late.
3. **Transaction-local pairing for xDai.** `executeAffirmation` emits the
   signature and the completion in one transaction with one `bytes32`. A
   pre-pass in `xdai::events::dispatch_transaction` pairs every hash-keyed
   `SignedForAffirmation` with a hash-keyed `AffirmationCompleted` carrying the
   same `bytes32`; the signature handler skips it and the completion handler
   inserts it, as the last mutation of its own `alter`, under the canonical key
   it already resolves. No new RPC. If the completion fails, the signature is
   applied nowhere and the transaction is reported failed and retried.

## Alternatives Considered

### Alternative 1: Write late confirmations directly from the event handler

Check in the handler whether the message is stored and insert the row there.
Rejected: a check-then-act race with the maintenance commit that cannot be
closed, final-storage writes outside the maintenance transaction, and a
database read per confirmation on the ingestion path.

### Alternative 2: Keep finalized entries as cold tombstones

Leave a finalized entry in `pending_messages` so late evidence restores it.
Rejected: `pending_messages` grows without bound, re-flushes amplify, and it
contradicts "finalized ⇒ evicted" that the pending-row diagnostics rely on.

### Alternative 3: Resolve every hash-keyed confirmation through its source receipt

Rejected: an RPC per hash-keyed signature, and it would pull the never-executed
bucket into canonical messages.

### Alternative 4: Execution-level attribution (P2)

Count only the canonical execution's signers. Rejected: it needs per-signature
execution provenance and filtering in both `consolidate` and the stored-wins
reconciliation, and the stored validator set would flip whenever a reindex picks
the other execution as canonical.

## Consequences

### Positive

- Every in-range signature that contributed to an execution is stored, whatever
  the scan order, batch size, retries or dispatch order.
- The ADR-014 negative consequence about orphaned hash-keyed confirmations is
  retired for co-located signatures without a per-confirmation RPC.
- Protocols that do not participate pay nothing: AMB and Avalanche inherit the
  `None` default and the attach step issues no statement. No schema change.

### Negative

- **Residual:** non-co-located hash-keyed signatures of an executed bucket stay
  pending. They exist only when `requiredSignatures > 1` and a modern nonce
  source is executed under the hash alias; there are zero known instances.
- A dirty `NotReady` xDai entry with confirmations costs one batched existence
  `SELECT` per maintenance cycle while it stays hot.
- A doubly executed message can hold more rows than `requiredSignatures`
  (Chiado nonces 0, 1 and 3: two rows at threshold 1). When one validator signed
  both executions, which transaction its single row names depends on processing
  order.

### Neutral

- `amb_messages_confirmations` has no read API; the semantics above are for
  internal consumers and diagnostics.
- An entry that also carries other evidence (for example a late
  `CollectedSignatures`) gets its confirmations attached but lingers until its
  own path resolves it, as before.

## References

- ADR-001 — the tiered buffer whose `restore` miss creates the late entry.
- ADR-005 — the failure ledger that retries a transaction whose paired
  completion failed.
- ADR-014 — first-seen canonical execution and the defaulted observation-channel
  pattern this reuses.
- `interchain-indexer-logic/src/message_buffer/types.rs` —
  `Consolidate::detached_confirmations`, `DetachedConfirmations`.
- `interchain-indexer-logic/src/message_buffer/maintenance.rs` —
  `plan_maintenance`, `commit_maintenance`, `resolved_hot_evictions`,
  `evict_resolved_keys`.
- `interchain-indexer-logic/src/message_buffer/persistence.rs` —
  `attach_detached_confirmations`.
- `interchain-indexer-logic/src/indexer/xdai/consolidation.rs` —
  `holds_only_confirmations`, `confirmation_models`.
- `interchain-indexer-logic/src/indexer/xdai/events.rs` —
  `pair_colocated_hash_signatures`, `collect_affirmation_log_facts`.
