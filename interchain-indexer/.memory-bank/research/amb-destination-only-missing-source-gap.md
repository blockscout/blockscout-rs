# AMB Destination-Only Consolidation and the Permanent Missing-Source Gap

## Scope

This note documents a production data-quality gap for AMB/Omnibridge
(`bridge_id = 1`): a subset of `crosschain_messages` rows reach
`status = Completed`/`Failed` with `src_tx_hash = NULL` and never get
backfilled, because the AMB consolidator finalizes and evicts them from the
message buffer before the matching source-side event has been seen. It covers
the AMB consolidation path only (`interchain-indexer-logic/src/indexer/amb/`)
and contrasts it with Avalanche's structurally different handling of an
analogous asymmetry (see `avalanche-unresolved-blockchain-ids.md` and
[ADR-010](../adr/010-unresolved-avalanche-destinations-and-protocol-metadata.md)
for the Avalanche side). General buffer/finalization mechanics are covered in
`message-lifecycle.md`; this note only adds the AMB-specific failure mode.

Not in scope: a code fix. This is a diagnosis note; see Open Questions for the
sizing of a possible fix.

This note also records two related-but-distinct findings surfaced during the
same investigation, kept here rather than split out because they share the
same tables/queries: (1) a not-yet-root-caused population of old, stuck
`Initiated` messages (Open Questions, Production Evidence Table B), and (2) a
confirmed-historical (not currently active) backlog of orphaned
`pending_messages` rows. Neither should be assumed to share a root cause with
`build_destination_only` just because they were found while investigating it.

## Short Answer

`build_destination_only`
(`interchain-indexer-logic/src/indexer/amb/consolidation.rs:223`) fires
whenever the buffer has seen a destination-chain execution event
(`AffirmationCompleted`/`RelayedMessage`) but no matching source-chain request
event (`UserRequestForSignature`/`UserRequestForAffirmation`) yet. That
function unconditionally sets `src_tx_hash = None` and marks the
`ConsolidatedMessage` `is_final: true`. Once `is_final` is true,
`classify_item` (`message_buffer/maintenance.rs:134`) treats it as
`ConsolidationOutcome::Complete`, the row is written, and the buffer entry for
that key is removed. If the source event is indexed later (source and
destination chains are indexed by independent catch-up cursors, so one can
lag the other), there is no remaining buffer entry to merge it into — the gap
is permanent, not "still catching up."

Avalanche has a structurally different consolidator for the equivalent
situation: `Consolidate::consolidate` for `avalanche/consolidation.rs` has an
explicit third arm — `(None, false)` ("no send, source chain configured") —
that returns `Ok(None)` and leaves the entry in the buffer to wait, rather
than finalizing on partial information
(`interchain-indexer-logic/src/indexer/avalanche/consolidation.rs:134`).
Avalanche's `is_final` also requires
`is_execution_succeeded && is_ictt_complete`, not merely "one side observed."
AMB has no equivalent wait branch for this case.

## Why This Matters

This is the underlying cause of a counter-consistency question raised against
the `stats` service: on `stats`'s `/api/v1/counters`,
`totalInterchainMessages` (all messages admitted by the interchain filter) did
not equal `totalInterchainMessagesSent + totalInterchainMessagesReceived`
(messages admitted by the filter **and** with the relevant `tx_hash`
populated) for `bridge_ids = [1]`, `home_chain_id = 100` (Omnibridge,
Gnosis↔Ethereum). See
`stats/stats/src/charts/counters/interchain/total_interchain_messages*.rs`
and `stats/stats/src/charts/db_interaction/filters/interchain.rs` for how
those three counters are defined; `stats` is reporting the underlying data
faithfully — the gap is upstream, in `interchain-indexer`.

Production evidence (2026-09-15) shows this is not merely "indexing hasn't
caught up yet": most affected rows are weeks to ~2 years old, `status =
Completed`, and `last_update_timestamp == init_timestamp` (never touched
again since the row was written). See Production Evidence below.

## Source-of-Truth Files

- `interchain-indexer-logic/src/indexer/amb/consolidation.rs`
  - `Consolidate::consolidate` (the four-way match on `(source,
    destination_execution)`, line ~21)
  - `build_source_led` (happy path, always sets `src_tx_hash`)
  - `build_destination_only` (the function at the center of this note)
  - `status_and_finality` (a *different*, non-buggy source of `Completed`
    status inside `build_source_led` — see Edge Cases)
- `interchain-indexer-logic/src/message_buffer/maintenance.rs` —
  `classify_item`, and the `Complete`/`Partial`/`NotReady`
  `ConsolidationOutcome` split that governs buffer eviction
- `interchain-indexer-logic/src/message_buffer/types.rs` — `is_final` field
  doc ("controls whether the entry can be removed from both tiers")
- `interchain-indexer-entity/src/codegen/crosschain_messages.rs` — column
  shapes (`src_tx_hash`/`dst_tx_hash` nullable, `status`, `init_timestamp`,
  `last_update_timestamp`)
- `config/full-mainnet/bridges.json` — confirms `bridge_id = 1` is
  `"AMB/Omnibridge"`, with a **fixed two-chain contract set** (chain 1 +
  chain 100 only)
- Contrast reference: `interchain-indexer-logic/src/indexer/avalanche/consolidation.rs:113-136`
  (the `(None, false)` wait arm and the `is_final` computation)
- Symptom surface: `stats/stats/src/charts/counters/interchain/total_interchain_messages.rs`,
  `total_interchain_messages_sent.rs`, `total_interchain_messages_received.rs`

## Key Types / Tables / Contracts

- `crosschain_messages.status` (`MessageStatus`: `Initiated`, `Completed`,
  `Failed`, `ReadyToClaim`)
- `crosschain_messages.src_tx_hash` / `dst_tx_hash` — nullable `bytea`
- `ConsolidatedMessage.is_final` — gates buffer eviction, not just DB write
- `ConsolidationOutcome::{Complete, Partial, NotReady}`
  (`message_buffer/maintenance.rs`)

## Step-by-Step Flow (the buggy path)

1. Destination-chain execution event (`AffirmationCompleted`/`RelayedMessage`)
   is indexed and reaches the buffer for a given `message_id` key.
2. The matching source-chain request event has not yet been indexed (source
   and destination chains are indexed independently; nothing guarantees
   ordering).
3. `consolidate()` takes the `(None, Some(destination_execution))` arm
   (`consolidation.rs:63`) → `build_destination_only`.
4. `build_destination_only` writes `src_tx_hash: None`, sets `status` from
   the destination event's boolean outcome (`Completed`/`Failed`), and
   returns `is_final: true` unconditionally.
5. `classify_item` sees `is_final == true` → `ConsolidationOutcome::Complete`.
6. The row is persisted and the buffer key is removed
   (`finalized_keys`/eviction machinery in `maintenance.rs`).
7. The source-chain event is indexed later (minutes, days, or — observed in
   production — up to ~2 years later). There is no buffered entry left for
   this key, so nothing re-triggers `consolidate()` for it, and
   `src_tx_hash` stays `NULL` forever.

## Invariants

- Once `ConsolidatedMessage.is_final == true`, the corresponding buffer entry
  is removed and will not be revisited by a later event for the same key.
- AMB's `Consolidate::consolidate` has **no branch that defers** when only a
  destination-side event is available; it always finalizes immediately
  (contrast: Avalanche's `(None, false)` arm explicitly waits).
- `build_destination_only` is reached from **two different call sites** with
  different legitimacy (see Edge Cases) — both currently share the same
  unconditional `is_final: true`.

## Failure Modes / Observability

**Canonical, direction-agnostic detection query** — does not assume a
`home_chain_id`, so it cannot miss the mirror-image case (see Edge Cases:
"the diagnostic query was home-chain-biased"):

```sql
SELECT
    '0x' || encode(native_id, 'hex') AS native_id,
    src_chain_id, dst_chain_id, status, init_timestamp,
    last_update_timestamp, now() - init_timestamp AS age
FROM crosschain_messages
WHERE bridge_id = :bridge_id
  AND status IN ('completed', 'failed')
  AND src_tx_hash IS NULL
ORDER BY init_timestamp;
```

This is sufficient on its own (no chain-id filter needed) because a given
`bridge_id`'s contract set is fixed per bridge config — for `bridge_id = 1`
that's exactly chains 1 and 100, so every affected row already falls inside
that pair.

A second, `home_chain_id`-scoped query mirrors what a specific `stats`
deployment's counters see (useful when the report comes in via a `stats`
discrepancy for a given filter, not as a general health check):

```sql
SELECT
    COUNT(*) FILTER (WHERE src_chain_id = :home AND src_tx_hash IS NULL) AS sent_missing_tx,
    COUNT(*) FILTER (WHERE dst_chain_id = :home AND dst_tx_hash IS NULL) AS received_missing_tx
FROM crosschain_messages
WHERE bridge_id = :bridge_id
  AND (src_chain_id = :home OR dst_chain_id = :home);
```

On `stats`, the symptom is `totalInterchainMessages !=
totalInterchainMessagesSent + totalInterchainMessagesReceived` for a filter
scoped to the affected bridge — but this symptom is specific to the `stats`
counters' `home_chain_id` framing (see Edge Cases) and does not by itself
prove the underlying `crosschain_messages` gap is one-directional. A
non-zero, non-shrinking `sent_missing_tx` (or `received_missing_tx`) count
where the affected rows have `status != Initiated` and old `init_timestamp`
is the fingerprint of this bug rather than of ordinary indexing lag (compare
against genuinely in-flight `Initiated` rows, which are expected to
eventually resolve).

## Edge Cases / Gotchas

- **`build_destination_only` is called from two places with different
  legitimacy** — this matters for any future fix:
  - The messageId-collision branch (`consolidation.rs:48`): the destination
    execution's real source body was displaced as an anomaly because it
    belongs to a *different* message. `is_final: true` is **correct** here —
    there genuinely is no matching source for this canonical row.
  - The direct `(None, Some(destination_execution))` arm
    (`consolidation.rs:63`): this is the race described in this note — the
    source event simply hasn't arrived yet, and might still.

  A fix must distinguish these two call sites rather than changing
  `build_destination_only` globally.

- **`bridge_id = 1` has a fixed, closed two-chain contract set** (chain 1 +
  chain 100, per `config/full-mainnet/bridges.json`) — unlike Avalanche's
  open N-chain model, there is no legitimate "source chain not indexed by
  this deployment" scenario for Omnibridge. Every destination-only row
  observed for this bridge is a genuine ordering race, not a structural
  scope gap. This simplifies a potential fix relative to Avalanche's
  `source_chain_is_unknown` distinction — AMB doesn't need an equivalent
  flag, only a give-up/staleness policy.

- This is **not** the same condition as `completed_message_without_indexed_source`
  in `interchain-indexer-logic/src/database.rs` tests, or
  `stats_projection_edge_uses_destination_when_source_chain_not_indexed` —
  those tests exercise the *projection* layer's handling of a message that
  already has `src_tx_hash = NULL` in storage; they do not indicate the gap
  is intentional at the consolidation layer. They will need re-examination if
  this is ever fixed upstream, since some of what they cover may still be a
  legitimate state (e.g. rows arriving via the collision branch above).

- Historical rows are **not self-healing**: the buffer holds no memory of
  already-evicted keys, so once a row is written via `build_destination_only`
  (direct arm), no later indexing progress will ever fill in `src_tx_hash`
  without an explicit backfill/replay job.

- **The original diagnostic query was home-chain-biased, not the underlying
  bug — and the mirror-image row is confirmed to exist in production.** The
  first query used to gather Production Evidence below mirrored the `stats`
  counters' `home_chain_id = 100` framing: `missing_src` required
  `src_chain_id = 100` and `missing_dst` required `dst_chain_id = 100`.
  `build_destination_only` itself has no such asymmetry — a mirror-image row
  (`src_chain_id = 1`, Ethereum-originated, Ethereum-side source event never
  indexed) satisfies neither condition and silently drops out of that
  query's result set. It is also invisible to the `stats` counters
  themselves: `sent` only looks at `src_chain_id = home`, and `received`
  doesn't check `src_tx_hash` at all, so such a row produces no `total !=
  sent + received` discrepancy either — it is just silently included in
  `total`/`received` with an unnoticed `NULL` `src_tx_hash`.

  **Correction (2026-09-15):** an earlier version of this note claimed a
  direction-agnostic re-check found no `src_chain_id = 1` rows. That was
  incorrect — no such re-check was actually performed at the time. The
  canonical direction-agnostic query (Failure Modes / Observability) run
  against production found 38 rows total: 37 `src_chain_id = 100` and
  **1 `src_chain_id = 1`** — `native_id
  0x000500004ac82b41bd819dd871590b510316f2385cb196fb0000000000026725`,
  `src_chain_id = 1 → dst_chain_id = 100`, `status = completed`, `init_timestamp
  = 2024-09-23 09:07:30` (~722 days old), same age bracket as the oldest
  `src_chain_id = 100` rows. See Production Evidence for the full table. This
  confirms the gap is genuinely bidirectional in production, not
  one-directional as an earlier version of this note (incorrectly) stated.

## Change Triggers

Update this note if:

- AMB consolidation (`indexer/amb/consolidation.rs`) changes how
  `is_final`/`build_destination_only` is decided, or gains a wait/give-up
  policy.
- The message buffer's `is_final`/eviction semantics change
  (`message_buffer/maintenance.rs`, `types.rs`).
- The `stats` interchain filter/counter semantics change in a way that
  changes how this gap surfaces (`stats/stats/src/charts/counters/interchain/`,
  `stats/stats/src/charts/db_interaction/filters/interchain.rs`).
- ~~The xDai Bridge indexer (`bridge_id = 3`, in development — see
  `xdai-bridge-protocol-and-indexing-fit.md`) is implemented; check whether
  its consolidator has an analogous destination-only/source-race path.~~
  **Checked (2026-09-15): xDai does not have this gap, by construction.**
  `xdai/consolidation.rs::consolidate` (line 31-48) requires either
  `source_request` (EthToGno) or `signature_request` (GnoToEth — itself a
  *source*-chain event, `CollectedSignatures` on Gnosis) before it will build
  a `crosschain_messages` row at all; with only `destination_execution`
  present it returns `Ok(None)` (see the
  `consolidate_without_source_request_is_not_yet_consolidatable` test) and
  the buffer keeps waiting, mirroring Avalanche's wait arm rather than AMB's
  immediate-finalize path. Consequently `src_tx_hash` is unconditionally
  `Some(...)` in the xDai message model (`consolidation.rs:64`) — there is no
  branch that can write a row with `src_tx_hash = NULL`. The code's own
  comment at `consolidation.rs:21-25` states this was deliberately modeled
  after the AMB `pending_messages` pattern and the Avalanche `SourceData`
  gate. xDai also has no messageId-collision handling / no
  `build_destination_only`-equivalent (identity is nonce-based, not
  `messageId`-hash-based, so there is no analogous collision to force an
  early, unconditional finalization for).

## Open Questions

- **Fix shape, not yet scoped as an ADR/implementation plan.** A prospective
  fix needs: (a) splitting `build_destination_only`'s two call sites so only
  the collision branch keeps unconditional `is_final: true`; (b) a
  wait-then-give-up policy for the direct arm, analogous to Avalanche's
  `(None, false)` → `Ok(None)` plus the buffer's existing hot/stale lifecycle,
  but AMB's `Consolidate::consolidate` currently has no access to
  elapsed-time/staleness context, so this likely means extending what the
  buffer passes in or moving the give-up decision to the buffer layer; (c)
  re-auditing the existing test suite in `amb/consolidation.rs` (~600 lines)
  and the related `database.rs` projection tests to separate "this encodes
  the bug" from "this is a legitimate case." Assessed as medium complexity —
  bounded to the AMB module plus a buffer-lifecycle extension, comparable in
  shape to the Avalanche ADR-010 effort, not a large redesign.
- **Backfilling already-affected production rows is a separate effort.**
  Buffer keys for the rows below are long evicted; repairing them requires a
  standalone replay/reconciliation job (re-fetch the specific source-chain
  event by `native_id`/`message_id` and update in place), plus care around
  already-computed `stats_processed`/`stats_messages`/`stats_asset_edges`
  projections for those rows so a later backfill doesn't double-count or
  desync stats aggregates.
- ~~Does the same shape of gap exist on the `ReceivedMessage`/incoming side,
  or only outbound-from-home as observed?~~ **Checked (2026-09-15): yes, it
  is bidirectional.** See Production Evidence, Table A — 1 of 38 rows has
  `src_chain_id = 1`. Not checked against other bridges (`bridge_id = 2`
  Avalanche has its own, different, already-documented asymmetry; `bridge_id
  = 3` xDai does not have this bug at all — see Change Triggers).
- **New, separate finding (2026-09-15), not yet root-caused: 20 old
  `Initiated` rows for `bridge_id = 1` with `dst_tx_hash IS NULL`, ages
  17–711 days, `last_update_timestamp = NULL`, both directions (16
  `100→1`, 4 `1→100`).** See Production Evidence, Table B.

  **This does not currently look like the same defect.** `build_destination_only`
  produces terminal (`is_final: true`) rows with a *permanently* missing
  `tx_hash`; these rows are `Initiated` (`is_final: false` in
  `status_and_finality`'s fallback arm), meaning they were never marked final
  and, per the buffer's design, should remain revivable: stale non-final
  entries get offloaded to the `pending_messages` cold tier
  (`message_buffer/persistence.rs::offload_stale_to_pending`) rather than
  discarded, and `MessageBuffer::restore()`
  (`message_buffer/buffer.rs:83`, invoked from `get_mut_or_default` on the
  next `alter()` call for the same key) is specifically designed to pull a
  stale entry back from `pending_messages` and merge in a later event. So
  "stuck at `Initiated` for 700 days" most likely means the downstream event
  (`CollectedSignatures` on Gnosis, or `RelayedMessage` on Ethereum) was
  never indexed for these messages at all — not that the buffer lost them.
  Candidate explanations, **not yet distinguished**: (a) genuinely stalled
  bridge transactions at the protocol level (validators never signed / no
  one ever relayed); (b) an indexing coverage gap for the relevant block
  range (check `indexer_failures`); (c) a key-correlation failure across the
  chain-100 contract version boundary (`bridges.json` lists two
  `amb_proxy`/`omnibridge_mediator` versions — v6 and v8 — at different
  addresses/start blocks for chain 100; worth checking whether any of the 20
  cluster around that boundary).

  **Checked (2026-09-15): all 16 `100→1` rows do have a live row in
  `pending_messages`** (confirmed by direct query — `pm.created_at` for all
  16 falls in 2026-09-01..09-07, i.e. recently re-offloaded, not from when
  the message was first created; see the orphan-backlog finding below for
  why `created_at` cannot be read as "first staged"). **This turned out to be
  a weaker signal than it first looked** — see the next bullet: ~97% of all
  `pending_messages` rows for this bridge belong to messages that are
  *already* `completed`/`failed`, so mere presence in `pending_messages`
  does not by itself indicate the buffer is meaningfully "still working on"
  a message. What *is* still informative here is that these 20 rows are
  genuinely `status = Initiated` in `crosschain_messages` (not finalized),
  so they remain open questions in their own right — just not resolved by
  the `pending_messages` check the way this note originally framed it.
  Root cause remains one of (a)/(b)/(c) above. Do not fold this into the
  `build_destination_only` fix scope until root-caused — it may need its
  own note.

- **New finding (2026-09-15): large historical backlog of orphaned
  `pending_messages` rows for `bridge_id = 1` — confirmed NOT an active
  bug.** A histogram of `pending_messages` rows joined to their
  `crosschain_messages` status showed the overwhelming majority already
  `completed`/`failed`:

  | age bucket | messages | initiated | ready_to_claim | other (completed/failed) |
  | --- | ---: | ---: | ---: | ---: |
  | 1h-1d | 2 | 0 | 2 | 0 |
  | 1d-7d | 1 | 0 | 1 | 0 |
  | 7d-30d | 911 | 2 | 2 | 907 |
  | 30d-180d | 6004 | 4 | 48 | 5952 |
  | >180d | 27504 | 14 | 680 | 26810 |

  Overall for `bridge_id = 1`: `total_messages = 73045`, `total_pending
  (pending_messages rows) = 34422`, `total_finalized (completed/failed) =
  72292`. So **~47% of every message ever processed for this bridge still
  has a leftover `pending_messages` row**, and of those, the ones flagged
  `other` above (33669 rows) should have been deleted by
  `remove_finalized_from_pending`
  (`message_buffer/persistence.rs:409`) when they finalized, and were not.

  **Confirmed NOT currently active**, though: checking whether any message
  finalized in the last 1 or 7 days still has an orphaned `pending_messages`
  row returned **0** both times. `remove_finalized_from_pending` runs inside
  the same DB transaction as `flush_to_final_storage`
  (`maintenance.rs:310-329`, `commit_maintenance`) and — per the code trace
  in this note — `ConsolidationOutcome::Complete` always returns before the
  staleness/offload branch (`maintenance.rs:190-202`), so the live runtime
  path looks correct on inspection and the data agrees: cleanup works today.

  This means the 33669 orphans are a **one-time historical backlog**, most
  likely predating whatever fixed cleanup (or predating the current
  `message_buffer`/`persistence` architecture entirely — e.g. an initial
  historical backfill that populated `crosschain_messages` through a
  different path than the live buffer, without exercising
  `remove_finalized_from_pending`). Not yet confirmed which; not otherwise
  investigated. Practical implication: this is a maintenance/cleanup task
  (safe to `DELETE FROM pending_messages` for rows whose
  `(message_id, bridge_id)` already has a `completed`/`failed`
  `crosschain_messages` row), not a defect requiring a code fix — but it
  should not be conflated with, or used as evidence for, the two findings
  above.

## Production Evidence

Both tables below are keyed by `native_id` (`0x` + hex, matching the
`messageId` used by the AMB contracts and the bridge explorer UI) rather than
the internal `id`, and use the direction-agnostic queries from Failure Modes
/ Observability — no `home_chain_id` bias, so both directions are covered.
Snapshot captured 2026-09-15 against the production indexer DB, `bridge_id =
1` (Omnibridge, Gnosis↔Ethereum).

### Table A — `build_destination_only` gap (this note's subject)

```sql
SELECT
    '0x' || encode(native_id, 'hex') AS native_id,
    src_chain_id, dst_chain_id, status, init_timestamp,
    last_update_timestamp, now() - init_timestamp AS age
FROM crosschain_messages
WHERE bridge_id = 1
  AND status IN ('completed', 'failed')
  AND src_tx_hash IS NULL
ORDER BY init_timestamp;
```

38 rows: 37 `src_chain_id = 100` (Gnosis-originated), **1 `src_chain_id = 1`**
(Ethereum-originated — row 2 below, confirming the gap is bidirectional).
Every row has `last_update_timestamp == init_timestamp` (never touched since
written) and `status` is `completed` or `failed` — not `Initiated` — so this
is not ordinary indexing lag. Ages are relative to capture time and grow
daily, since these rows are not self-healing.

| native_id | route (src→dst) | status | init_timestamp (UTC) | age at capture |
| --- | :---: | --- | --- | --- |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000016750 | 100→1 | completed | 2024-09-23 09:05:11 | 722d |
| 0x000500004ac82b41bd819dd871590b510316f2385cb196fb0000000000026725 | 1→100 | completed | 2024-09-23 09:07:30 | 722d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000166e5 | 100→1 | completed | 2024-09-23 22:11:23 | 721d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000016743 | 100→1 | completed | 2024-09-24 22:07:47 | 720d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000164fa | 100→1 | completed | 2024-10-01 09:37:35 | 714d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000016706 | 100→1 | completed | 2024-10-02 09:03:11 | 713d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000016697 | 100→1 | completed | 2024-10-13 11:17:35 | 702d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000012dfe | 100→1 | completed | 2024-11-07 12:50:11 | 677d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000015276 | 100→1 | completed | 2024-11-07 12:55:47 | 677d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000153dd | 100→1 | completed | 2024-11-20 11:23:59 | 664d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000011daa | 100→1 | completed | 2024-11-26 03:37:47 | 658d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000166a9 | 100→1 | completed | 2024-11-26 18:26:11 | 657d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000015726 | 100→1 | completed | 2024-12-03 10:14:47 | 651d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000156a6 | 100→1 | completed | 2024-12-07 04:09:47 | 647d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000147ed | 100→1 | completed | 2024-12-07 11:18:23 | 647d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000013198 | 100→1 | completed | 2024-12-14 21:33:59 | 639d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001636e | 100→1 | completed | 2024-12-16 08:52:35 | 638d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000016370 | 100→1 | completed | 2024-12-16 08:52:35 | 638d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000015165 | 100→1 | completed | 2025-01-04 04:58:35 | 619d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000106c4 | 100→1 | completed | 2025-01-06 01:30:47 | 617d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000000a507 | 100→1 | completed | 2025-01-11 16:16:23 | 612d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000000f1c7 | 100→1 | completed | 2025-01-25 13:36:35 | 598d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000155f7 | 100→1 | completed | 2025-01-25 14:37:47 | 598d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000159ee | 100→1 | completed | 2025-02-22 04:14:59 | 570d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000154f2 | 100→1 | completed | 2025-04-03 12:33:35 | 530d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000014a8e | 100→1 | completed | 2025-04-03 20:02:59 | 529d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001622a | 100→1 | completed | 2025-04-14 17:54:59 | 519d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001632a | 100→1 | failed | 2025-07-17 01:31:23 | 425d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000153d1 | 100→1 | completed | 2025-09-21 22:33:23 | 358d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000014a42 | 100→1 | completed | 2025-10-04 21:58:47 | 345d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000014383 | 100→1 | completed | 2025-10-07 12:53:23 | 343d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000000f79c | 100→1 | completed | 2025-11-17 23:37:23 | 301d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000111b6 | 100→1 | completed | 2026-05-25 19:38:59 | 112d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000008a6e | 100→1 | completed | 2026-07-27 09:39:59 | 50d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001e8ab | 100→1 | completed | 2026-08-29 15:37:47 | 17d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001e8af | 100→1 | completed | 2026-08-29 15:38:59 | 17d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001e8b5 | 100→1 | completed | 2026-08-29 15:40:35 | 17d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001ea84 | 100→1 | completed | 2026-09-10 09:51:11 | 5d |

### Table B — stuck `Initiated`, ages 17–711 days (separate, not yet root-caused — see Open Questions)

```sql
SELECT
    '0x' || encode(native_id, 'hex') AS native_id,
    src_chain_id, dst_chain_id, status, init_timestamp,
    last_update_timestamp, now() - init_timestamp AS age
FROM crosschain_messages
WHERE bridge_id = 1
  AND status = 'initiated'
  AND dst_tx_hash IS NULL
ORDER BY init_timestamp;
```

20 rows: 16 `100→1`, 4 `1→100` — also bidirectional. All have
`last_update_timestamp = NULL`.

| native_id | route (src→dst) | init_timestamp (UTC) | age at capture |
| --- | :---: | --- | --- |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001690f | 100→1 | 2024-10-04 00:04:40 | 711d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000016923 | 100→1 | 2024-10-04 00:50:00 | 711d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000016ce1 | 100→1 | 2024-10-25 03:09:15 | 690d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000016e32 | 100→1 | 2024-10-27 10:39:40 | 688d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000017240 | 100→1 | 2024-11-07 05:38:30 | 677d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000173a9 | 100→1 | 2024-11-12 00:15:10 | 672d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000018233 | 100→1 | 2025-01-18 04:10:10 | 605d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe70000000000018259 | 100→1 | 2025-01-18 17:57:10 | 605d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000191f1 | 100→1 | 2025-04-01 12:22:30 | 532d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000192fc | 100→1 | 2025-04-07 03:50:05 | 526d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe700000000000197f0 | 100→1 | 2025-04-29 11:32:55 | 504d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001a91f | 100→1 | 2025-07-30 12:26:20 | 412d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001ab9f | 100→1 | 2025-08-11 11:42:05 | 400d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001bc37 | 100→1 | 2025-11-12 23:58:55 | 306d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001dace | 100→1 | 2026-06-01 12:22:20 | 106d |
| 0x00050000a7823d6f1e31569f51861e345b30c6bebf70ebe7000000000001dad3 | 100→1 | 2026-06-01 15:57:20 | 106d |
| 0x000500004ac82b41bd819dd871590b510316f2385cb196fb000000000002f786 | 1→100 | 2026-07-21 12:56:11 | 56d |
| 0x000500004ac82b41bd819dd871590b510316f2385cb196fb000000000002f787 | 1→100 | 2026-07-21 13:41:23 | 56d |
| 0x000500004ac82b41bd819dd871590b510316f2385cb196fb000000000002fd8f | 1→100 | 2026-08-29 16:20:23 | 17d |
| 0x000500004ac82b41bd819dd871590b510316f2385cb196fb000000000002fd91 | 1→100 | 2026-08-29 16:22:35 | 17d |

The 4 `1→100` rows above were invisible to the earlier `home_chain_id =
100`-scoped check for the same reason as Table A's mirror row: they have
`dst_chain_id = 100`, not `1`, so a query scoped to `dst_chain_id = 1` misses
them entirely — another instance of the home-chain-bias trap (see Edge
Cases).
