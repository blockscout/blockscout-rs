# Gotchas

Non-obvious traps and their solutions.

## Message Finality is Complex

**Symptom:** Messages stuck in "Initiated"/"Partial" status despite execution events arriving, accumulating permanently in `pending_messages`.

**Root cause:** A message is NOT final if:
- Execution failed (can be retried via `retryMessageExecution()`)
- ICTT transfer incomplete

Before the incoming-ICTT-reconstruction task, "incomplete" meant "the
destination side produced no `TokensWithdrawn`/`CallSucceeded`/`CallFailed`",
full stop — which is wrong for two message shapes that will *never* get one:
an incoming ICTT message from a chain not configured for the bridge (no
`send`, so no destination-paired source side either), and a fully indexed
multi-hop first leg whose home chain routes onward (`TokensRouted`) instead
of crediting a recipient. Both stayed `Partial` forever, were never
stats-projected, and never left `pending_messages`.

**Fix:** The ICM payload (`TeleporterMessage.message`, decoded by
`ictt_payload.rs`) is what discriminates this: `SINGLE_HOP_SEND` /
`SINGLE_HOP_CALL` mean a destination credit is expected and completeness still
waits for it; `REGISTER_REMOTE` / `MULTI_HOP_SEND` / `MULTI_HOP_CALL` mean no
credit will ever arrive for this message id, so the transfer is complete as
soon as the available message evidence is present — for an unknown-source
message that has no source-side event at all, this means the receive or
failed-execution evidence is what completeness rests on, not a source side
that will never exist. This classification runs on every
`consolidate()` call that has any payload source (`send` | `receive` |
`execution = Failed`), not only when reconstruction can build a row. Check
`consolidation.rs`'s `ictt_completeness` / `classify_payload` for the logic.

To confirm this at runtime against a live database, see
`.memory-bank/runbooks/runtime-verification.md` queries F (incoming ICTT
reconstruction is landing) and G (`pending_messages` backlog trend).

---

## Events Filtered for Unconfigured Chains

**Symptom:** Events from a chain are not being indexed, only trace-level logs visible.

**Root cause:** Avalanche message filtering now has two sequential bridge-level filters:
1. `process_unknown_chains` (chain-config filter)
2. `home_chain_id` (endpoint narrowing filter)

The `chain_ids` HashSet is built from all chains that have:
1. A contract listed in `bridges.json` for this bridge
2. A chain configuration in `chains.json` with at least one enabled RPC provider

Filtering happens in 4 event handlers:
- `handle_send_cross_chain_message()` - checks **destination_chain_id**
- `handle_receive_cross_chain_message()` - checks **source_chain_id**
- `handle_message_executed()` - checks **source_chain_id**
- `handle_message_execution_failed()` - checks **source_chain_id**

Events are skipped when they fail either filter:
- both endpoints unknown are always skipped
- one-known/one-unknown requires `process_unknown_chains: true`
- if `home_chain_id` is set, at least one endpoint must equal it (even for configured-chain <> configured-chain messages)

**Fix:**
1. Add the chain to the bridge's configured chains in `bridges.json` (and ensure it has RPC config in `chains.json`)
2. OR set `process_unknown_chains: true` to allow one-known/one-unknown messages
3. Optionally set `home_chain_id: <chain_id>` to narrow to messages touching a specific chain
4. Check trace-level logs for "filtered by bridge chain policy"

**Note:** The filtering happens BEFORE messages enter the buffer, so unfiltered messages never reach consolidation or database layers.

**Testing note:** When every log in a batch is filtered out by bridge policy, no buffer mutation happens, so `indexer_checkpoints` may remain empty for that chain/bridge. In strict-filter tests, prefer asserting message/pending absence (or blockchain-ID resolution) instead of waiting for checkpoint rows.

---

## Checkpoint Stall When All Events Are Perpetually Filtered

**Symptom:** After a service restart, a chain/bridge pair re-processes blocks it already saw, wasting RPC calls until it catches back up.

**Root cause:** Checkpoint advancement depends on `touched_blocks` recorded during `buffer.alter()` calls. If bridge filtering rejects every event for a chain/bridge pair, no `alter()` happens, no `touched_blocks` are recorded, and the checkpoint for that pair never advances. During normal runtime the `LogStream` progresses in memory, so there is no livelock. But on restart, the indexer resumes from the stale checkpoint and replays already-filtered blocks.

**When this happens:** A chain/bridge pair where **all** messages are perpetually filtered — e.g., a chain that only communicates with unconfigured chains under `process_unknown_chains: false`, or a chain whose messages never touch `home_chain_id`.

**Impact:** No data loss or correctness issue. The cost is wasted RPC calls on restart proportional to how far the LogStream progressed beyond the stale checkpoint. Self-correcting once any event passes filtering and triggers a `buffer.alter()`.

**Mitigation:** If a chain/bridge pair is known to produce only filtered events, consider removing it from the bridge's contract config rather than relying on runtime filtering to discard everything.

---

## AMB Source and Destination Events Can Arrive Out of Order

**Symptom:** AMB/Omnibridge messages are indexed, but transfers are missing for
one direction, especially when destination-chain execution is processed before
the source-chain request during catchup.

**Root cause:** AMB indexing merges independent chain streams. Destination
events such as `RelayedMessage` / `AffirmationCompleted` can be observed before
the matching `UserRequestForSignature` / `UserRequestForAffirmation`. Transfer
reconstruction must therefore not depend on having both sides in hand at the
same time.

**Fix:** Persist source-side `TokensBridgingInitiated` (`source_transfer`) and
destination-side `TokensBridged` (`destination_transfer`) details into the
buffered AMB message as each is observed. The transfer row is built at
consolidation from whichever sides are present; a side whose event has not yet
arrived is left NULL (see *AMB Transfer Sides Are Nullable and Never Mirrored*).
The transfer is **not** reconstructed from the AMB application calldata — see
[ADR-003](adr/003-amb-event-based-transfers.md).

Persistence must preserve this order independence too. A destination-only
finalized entry can be evicted, then a later source-only partial flush can hit
the same `(message_id, bridge_id, index)` transfer key. `crosschain_transfers`
conflict handling must merge nullable side columns with the stored row rather
than blindly updating them, or the late partial side will clear token/amount
data that was already extracted from the opposite-side event.

---

## AMB Collision Replacement Must Delete Before Insert

**Symptom:** After detecting an AMB `messageId` collision, the canonical
`crosschain_messages` row still contains fields from the displaced body
(`src_tx_hash`, `payload`, `sender_address`), or its `crosschain_transfers` row
still contains the displaced source-side token/amount.

**Root cause:** The normal AMB persistence path intentionally uses `COALESCE`
and nullable-side transfer merging so out-of-order source/destination events can
enrich each other. That merge policy is wrong for a confirmed collision: the
old row belongs to a different AMB body and must not be enriched.

**Fix:** Collision-produced canonical rows must request replacement. The
maintenance flush deletes the existing `(id, bridge_id)` row first, relying on
FK cascade to remove old transfers/confirmations, then inserts the executed body
and anomaly rows in the same transaction. Do not use replacement for ordinary
late source/destination merges.

---

## AMB Transfer Sides Are Nullable and Never Mirrored

**Symptom:** `crosschain_transfers` rows where `token_src_address == token_dst_address`
(and identical `src_amount`/`dst_amount`) for AMB/Omnibridge — i.e. a "transfer"
that looks like it moved the same token to itself.

**Root cause (historical):** `token_src_address`, `token_dst_address`,
`src_amount`, `dst_amount` were once `NOT NULL`. When a side was unknown, the
indexer substituted the only token it had into both columns. The substituted
value came from the AMB application calldata, whose token is the *native-chain*
token (source token for `handleBridgedTokens*`, but the **destination** token
for `handleNativeTokens*`), so mirroring conflated the two sides and corrupted
stats projection.

**Current behavior:** Those four columns are **nullable**. Each transfer side is
populated *only* from its own bridge event — source from `TokensBridgingInitiated`,
destination from `TokensBridged`. A side whose event has not been observed is
left **NULL**; it is never mirrored from the opposite side. So
`token_src_address == token_dst_address` now means a genuine same-address pair,
not a placeholder.

**Implications:**
- Readers must treat all four columns as optional. The proto layer emits
  `source_token`/`destination_token = None` and omits the amount when NULL.
- Stats projection skips a NULL endpoint (no token-key enrichment, no asset link
  for that side) and falls back to the known side's amount for edge volume; see
  `stats/projection.rs`.
- Old mirrored rows persist until reindexed — this change is go-forward only.
- The down migration backfills NULLs with a zero-address / zero-amount sentinel
  (not by mirroring) to restore `NOT NULL`.

See [ADR-003](adr/003-amb-event-based-transfers.md) and
`research/amb-omnibridge-token-reconstruction.md`.

---

## AMB Queued Events Must Preserve Their Emitting Chain

**Symptom:** `indexer_checkpoints.realtime_cursor` for Ethereum can jump to a
Gnosis block number, causing Ethereum realtime polling to wait forever because
the cursor is higher than the Ethereum latest block.

**Root cause:** AMB validator/signature events may be observed before the
matching source request and temporarily queued by `message_hash`. Any queued
event must store the chain that emitted it. If the event is later drained using
the source request's current chain context, the buffer records the queued
event's block number under the wrong chain and checkpoint maintenance persists
that wrong `(bridge_id, chain_id)` cursor.

**Fix:** Keep cursor attribution tied to the physical log source chain, not the
AMB header source/destination chain or the context that drains a pending queue.

---

## AMB Home/Foreign Side Comes From Proxy ABI Events

**Symptom:** AMB/Omnibridge indexing fails during startup with an error about a
missing Home or Foreign chain, or events are subscribed on the wrong side.

**Root cause:** AMB configs do not hardcode Ethereum/Gnosis chain IDs. The
indexer infers each configured `amb_proxy` as Foreign or Home from its ABI event
set:
- Foreign proxy ABI must include `UserRequestForAffirmation` and `RelayedMessage`
- Home proxy ABI must include `UserRequestForSignature`, `AffirmationCompleted`,
  validator signature events, and `CollectedSignatures`

The bridge config must contain exactly one Home and one Foreign proxy for
destination-side event annotation and collected-signature routing.

**Fix:** For non-mainnet AMB deployments, keep the side-specific proxy ABI
events in `bridges.json` / `bridges-testnet.json`. Do not rely on numeric chain
IDs to identify Home or Foreign.

---

## AMB Header Sender Is Not The Source Transaction Initiator

**Symptom:** AMB/Omnibridge `crosschain_messages.sender_address` can show the
AMB message header sender instead of the address that initiated the source-chain
transaction.

**Root cause:** AMB receipts include the EVM transaction origin (`receipt.from`),
but the shared EVM receipt helper currently drops it before AMB event dispatch.
Source request consolidation then writes `source_event.header.sender` into the
canonical message row. The AMB header sender/executor are protocol identity
fields and are still required for message matching and collision detection, but
they are not a substitute for the source transaction initiator.

Recipient has a separate semantic trap: AMB message destination is the AMB
message executor, not the Omnibridge transfer recipient. `TokensBridged.recipient`
belongs only to the transfer row (`crosschain_transfers.recipient_address`), not
to the canonical AMB message row.

**Fix:** Thread `receipt.from` through the AMB source request event and write it
to `crosschain_messages.sender_address` for source-led rows. Preserve AMB header
sender/executor separately for collision checks. For AMB message recipient,
write the message executor only: destination execution executor when available,
otherwise the source header executor. Do not fulfill message `recipient_address`
from `destination_transfer.recipient`. Existing rows need reindexing to change.

---

## Token Info Caches Errors

**Symptom:** Token metadata fetch fails once, then never retries.

**Root cause:** `TokenInfoService` caches fetch errors with a TTL to avoid hammering failed endpoints.

**Fix:** Wait for error cache TTL to expire, or restart service. Check `token_info/service.rs` for cache settings.

---

## Token Info Is Eventually Consistent and Reads Can Write Back

**Symptom:** API returns only token address with empty metadata on the first
request, or a token icon appears later without any re-indexing run.

**Root cause:** `TokenInfoService` returns a placeholder model immediately on
cache / DB miss and fetches metadata in the background. Separately, request-time
reads for an existing token can fetch a missing icon and persist it back into
`tokens`.

**Fix:** Treat token metadata as async enrichment, not as canonical indexed
state. Check provider config, `onchain_retry_interval`, and
`token_info/service.rs` when debugging token metadata gaps.

---

## Stats Edge Amount Side Must Follow Indexed Source Presence

**Symptom:** `stats_asset_edges.amount_side` flips to destination for a source-indexed transfer just because source token info was not fetched yet.

**Root cause:** Edge side selection is sticky, so choosing it from token metadata availability couples aggregation semantics to an async enrichment race. The stable provenance signal is `crosschain_messages.src_tx_hash`: when it is present, the source chain was indexed and source amounts should be used even if source token decimals are still missing.

**Fix:** For new stats edges, prefer `EdgeAmountSide::Source` whenever the parent message has `src_tx_hash`; only fall back to destination when the source chain truly was not indexed. Keep decimals enrichment separate from side selection.

---

## Stats Transfer Backfill Matches Failed AMB Projection Eligibility (RESOLVED)

**Status:** Resolved by the bridge-filtered projected-stats work
(`m20260720_120000_add_read_filters_and_bridge_stats`).

**Previous symptom:** After clearing stats projections, resetting
`stats_processed`, and running startup backfill, bridged-token stats previously
produced for terminal failed AMB messages went missing, because the transfer
candidate query in `backfill_stats_projection_round()` filtered the parent
message to `Completed` only while live `project_transfers_batch()` also accepts
failed AMB.

**Invariant now enforced:** Live projection and historical backfill share a
single eligibility predicate — `finalized_message_stats_condition()` in
`stats/projection.rs`, exposed as `pub(crate)`. Both the message backfill query
and the transfer backfill query in `database.rs` call it, and the transfer query
joins `crosschain_transfers -> crosschain_messages -> bridges` before applying
it (still requiring the parent's `stats_processed > 0` and the transfer's own
marker to be zero). A message/transfer counts when its (parent) message is
`Completed` (any bridge) or `Failed` on an AMB bridge; failed non-AMB rows stay
excluded on both paths. Regression tests
(`stats_backfill_failed_amb_included_non_amb_excluded_idempotent` and
`stats_projection_excluded_rows_still_excluded_from_daily_and_all_time`) cover
this. A full rebuild after a projection-invalidating migration therefore no
longer silently drops failed-AMB aggregates.

---

## Bridge Name Cache Has No Negative Caching

**Symptom:** Repeated DB queries for non-existent bridge IDs.

**Root cause:** `InterchainDatabase` caches known bridge names but doesn't cache "not found" results.

**Fix:** Ensure bridge IDs in messages always exist in database. Consider adding negative caching if this becomes a performance issue.

---

## SeaORM Entity Regeneration Overwrites Manual Changes

**Symptom:** Custom entity code disappears after `just generate-entities`.

**Root cause:** `sea-orm-cli generate entity` overwrites `src/codegen/`. Manual additions should go in `src/manual/`.

**Fix:** Put customizations in `interchain-indexer-entity/src/manual/`, not `codegen/`.

---

## PostgreSQL Bind Parameter Limit

**Symptom:** "too many bind variables" error on large inserts.

**Root cause:** PostgreSQL limits bind parameters to 65535 per statement.

**Fix:** Use `batched_upsert()` or `run_in_batches()` from `bulk.rs`. Calculate batch size as `65535 / columns_per_row`.

---

## Indexer Cleanup Guard Runs on Panic

**Symptom:** Indexer state shows "Idle" after a panic, but internal state may be inconsistent.

**Root cause:** `IndexerCleanupGuard` implements `Drop` to ensure state transitions even on panic.

**Fix:** After a panic, the indexer may need a full restart. Check logs for the panic cause before restarting.

---

## `started_at_block = NULL` Means "Index from Genesis"

**Symptom:** Indexing starts at block `0` when `started_at_block` is unset.

**Root cause:** `bridge_contracts.started_at_block` is nullable; `None` maps to `.unwrap_or(0)` in `BridgeContractConfig`.

**Fix:** Set `started_at_block` only for non-genesis starts. Treat `NULL` as expected (no warning).

---

## Cross-Bridge Resolver Persistence Leaks

**Symptom:** Bridge B (with `process_unknown_chains: false`) resolves a previously unknown blockchain ID on the first lookup without hitting the Avalanche Data API.

**Root cause:** `BlockchainIdResolver` writes to the shared `chains` table and `avalanche_icm_blockchain_ids`. If bridge A has `process_unknown_chains: true` and discovers chain C, bridge B benefits from the cached resolution on subsequent lookups. The resolver cache and persistence layer are global, but the filtering decision (`should_process_message`) is per-bridge.

**Impact:** This is benign for filtering — bridge B still applies its own `chain_ids` set and rejects the message. The only effect is that bridge B avoids a Data API call. The `chains` table may contain entries created by one bridge's discovery policy that wouldn't exist under another bridge's stricter policy.

**Fix:** No fix needed. This is expected behavior. Be aware that the `chains` table reflects the union of all bridges' discovery activity, not any single bridge's configured set.

---

## Stats Asset Mapping Conflicts Merge; Only Same-Chain Collisions Skip

**Symptom:** A transfer whose two endpoints already map to two different
`stats_assets` no longer stalls as a fragmented pair — the components are
merged automatically, visible as `interchain_indexer_stats_asset_merges_total{outcome="merged"}`
increasing. The skip that remains is rarer: a warning like `stats projection:
stats asset already has a different token on the destination chain; skipping
transfer` (or `...two different tokens on one chain; skipping`), paired with
`interchain_indexer_stats_asset_merges_total{outcome="refused_chain_collision"}`.
Separately, `stats projection: skipping transfer due to stats_asset_edges
decimals mismatch` paired with `interchain_indexer_stats_edge_decimals_conflict_total`
is a different, non-corrupting skip — see below.

**Root cause:** Asset identity is an incrementally discovered connected-component
problem — two complete transfers on fully indexed chains can legitimately form
disjoint components (`{A,B}` and `{C,D}`) that a later `B→C` transfer must join.
`ensure_asset_for_transfer` resolves this via `merge_assets`: a transactional,
validate-then-mutate union (weighted — the component with more linked tokens
wins, ties go to the lower id) that repoints the loser's `stats_asset_tokens`,
`stats_asset_edges` (folding amounts, rescaling for a decimals difference), and
`crosschain_transfers.stats_asset_id`, then deletes the loser `stats_assets`
row, all inside the same transaction as the triggering transfer. The only
genuine refusal left is a merge that would place two different tokens of one
chain into one `stats_asset` (a `stats_asset` can hold at most one token per
chain) — that case cannot be resolved automatically and cannot be forced
without corrupting the chain-uniqueness invariant.

A decimals conflict is a separate, unrelated skip on the *counting* path:
by the time it fires, this transfer's asset identity is already resolved
unambiguously (directly or via a merge) — the conflict is only about whether
this transfer's amount can be safely folded into the edge aggregate. It never
aborts the batch (task Decision 7) and, since identity succeeded here, the
transfer still links its resolved `stats_asset_id`.

**Impact:** Canonical `crosschain_messages` / `crosschain_transfers` rows are
never at risk in any of these paths. For a successful merge, the database
changes: the winner asset absorbs the loser's tokens, edges, and transfers,
and the loser row is gone — by design, not a side effect to repair. For a
refused chain-collision merge, the transaction leaves the database
byte-identical: nothing is mutated beyond marking the triggering transfer
`stats_processed += 1` with `stats_asset_id` left `NULL`. For a decimals
conflict, `stats_processed += 1` and `stats_asset_id` is set to the resolved
asset, with no `stats_asset_edges` contribution.

Read `crosschain_transfers.stats_asset_id` accordingly: `NULL` means identity
is genuinely unknown or ambiguous (the chain-collision refusal is the only
remaining case); a set `stats_asset_id` with `stats_processed > 0` and no
corresponding edge contribution means identity is known but this transfer's
amount was not counted (the decimals-conflict case). Either way the skipped
row is marked processed so it does not re-warn every maintenance cycle;
ongoing warnings usually mean new transfers keep hitting the same bad token
data or a backfill is processing historical rows.

**Fix:** A successful merge needs no manual repair — it already is the repair.
A chain-collision refusal is a genuine data problem: verify the token address
recorded per chain for both components (a token's address was likely
misattributed to the wrong chain), fix the source data, then reset the
affected transfers' `stats_processed` for re-projection. For local
development, a fresh reindex may be simpler.

To confirm this at runtime against a live database, see
`.memory-bank/runbooks/runtime-verification.md` queries A (split-asset
detector) and B (refusal legitimacy check).

---

## Stats Eligibility Is About Observability, Not Protocol Terminality

**Symptom:** A bridged token appears as two one-token `stats_assets` (one per
chain); or a processed transfer is later found with both token endpoints while
its assigned asset contains only one of them; or a whole chain pair is missing
from bridged-token and message-path stats even though canonical rows exist.

**Root cause:** two different kinds of incompleteness get conflated.

- *Protocol terminality* — AMB destination execution is terminal even when the
  source-chain request has not been observed yet. Accepting that
  destination-only transfer creates a singleton asset and marks the transfer
  processed; a later source-side upsert fills the nullable canonical columns but
  preserves `stats_processed` / `stats_asset_id`, so stats never reconsiders the
  now-complete pair.
- *Observability* — a message whose counterpart chain is not configured for its
  bridge can never be confirmed. Judging it by `status = Completed` defers it
  forever, so its data disappears from stats instead of being available as an
  opt-in slice.

**Invariant:** the stats layer decides eligibility from one question — *can the
missing evidence still arrive?* It can exactly when the chain that would produce
it is indexed **by that bridge** (it has a configured contract there). Nothing in
this rule may branch on bridge type. The full rationale, the indexer contract it
depends on, and the rejected alternatives are in
`adr/004-stats-observability-horizon-and-asset-union-find.md`.

- missing token endpoint, counterpart chain indexed → defer;
- missing token endpoint, counterpart chain unindexed → commit to what is known;
- missing destination confirmation, destination chain indexed → defer;
- missing destination confirmation, destination chain unindexed → count now.

The indexed-chain set comes from the **in-memory config**, per bridge. Do not
read it from `bridge_contracts`: startup backfill runs before
`upsert_bridge_contracts`, so a DB-derived set is stale exactly when backfill
needs it. The same set must reach live projection and both backfill candidate
queries — a divergence either loses rows or makes backfill loop forever.

**A bridge removed from the config is not the same as a bridge with no
contracts** (added 2026-07-28). `may_observe` answers `true` for a bridge *absent*
from the set (permissive: defer, and keep showing its rows) and `false` for a
bridge *present* with an empty set (restrictive: count now, hide its rows).
Removing a bridge from `bridges.json` must therefore commit nothing and hide
nothing — `upsert_bridges` never deletes the `bridges` row and nothing filters on
`bridges.enabled`, so the rows stay joinable and only their classification could
change. The branch is unreachable on the live path (no indexer ⇒ no flushes) but
**reachable on startup backfill**, which scans every `stats_processed = 0` row
regardless of which indexers run. `map.get(&bridge_id).is_some_and(..)` is the
plausible-looking bug here; it must be `map_or(true, ..)`.

**Counting and identity are separate concerns.** `stats_processed` guards
counting only: additive, exactly once, never reversed. Asset identity — linking a
newly known token endpoint, merging two asset components — is idempotent
maintenance that may re-run for an already-counted transfer. Filling a missing
side always requires a flush of that canonical key, so the live path must run
identity maintenance for every flushed key, not only for entries whose incoming
buffer item is `is_final`.

**Warning:** never reset `stats_processed` after enrichment. `stats_asset_edges`
updates are additive and would double count the previous projection. Late repair
must go through identity maintenance, never through re-counting.

**Consequence to expect:** a transfer counted while its destination chain was
unindexed stays counted after that chain is added, even if it turns out the
movement never completed. That is an accepted inaccuracy for the opt-in
unindexed slice, not a bug. A transfer whose counterpart chain *is* indexed but
whose evidence is permanently lost (AMB `messageId` collision, history older than
the configured start block) stays deferred forever by design — a marker-zero row
with a NULL endpoint is not a backfill backlog.

To confirm this at runtime against a live database, see
`.memory-bank/runbooks/runtime-verification.md` queries D (deferred
transfers, classified by reason) and E (unindexed-chain edges).

---

## Recoverable Message Fields Are Not A "Never Mirror" Case

**Symptom:** For a message arriving from a chain the bridge does not index,
`crosschain_messages.sender_address`, `recipient_address`, and `payload` were
all NULL even though the `ReceiveCrossChainMessage` (or
`MessageExecutionFailed`) log the destination chain delivered carries all
three directly. On a live database this was 100% of unindexed→indexed
messages (13,865 rows on one bridge), versus 0% of indexed-source messages.
It looked intentional because a regression test asserted the NULLs as an
invariant.

**Root cause:** `SourceData::from_receive` / `from_execution` in
`indexer/avalanche/consolidation.rs` built with `..Default::default()`,
discarding all three fields — even though the very same `TeleporterMessage` is
read a few lines away in the same function, for payload classification and
ICTT reconstruction. This traces back to `4c7198d1` (the `SourceData` block was
byte-identical on `main` for a long time), but commit `9329320c` mistakenly
*codified* it: it added a test and a comment asserting the NULLs as correct,
borrowing ADR-003's "never mirror an unknown side" language and misapplying it
to fields that are not mirroring at all.

**The line ADR-003 actually draws:** ADR-003 forbids copying *one side's*
value into the *other side's* column when the true value for that side is
genuinely unknown and directionally ambiguous — its motivating case is the AMB
calldata `_token`, which means the source or the destination token depending on
which mediator function was called, so there is no way to know which side it
belongs to. Reading `originSenderAddress` / `destinationAddress` / `message`
out of a delivered, Warp-verified ICM message is not that: it is an
unambiguously named field the message carries **about itself**, observed from
an equally authoritative point (the destination chain's own receipt). The test
is whether a value is *ambiguous* or merely *late*. `src_tx_hash` is the one
field this reasoning does not rescue: it is a fact about a transaction on a
chain never observed, and no field anywhere carries it — it must stay NULL in
every fallback path.

**Fix:** `from_receive` and `from_execution`'s `Failed` arm now populate
`sender_address`, `recipient_address`, and `payload` from the delivered
`TeleporterMessage`; `from_execution`'s `Succeeded` arm is unchanged because
`MessageExecuted` carries only `messageID` + `sourceBlockchainID` — genuinely
nothing to recover there. This is the same "fill it if any single indexed
chain's events carry it" rule ADR-004 Decision 4 already states for
`crosschain_transfers` sides, generalized to `crosschain_messages` fields; see
that ADR's Decision 4.

**Before writing off a NULL as "the other side is unknown," check whether the
value is actually ambiguous or just sitting in an event you haven't read yet
in this code path.**

**Operational consequence:** this fix changes `stats_chains` unique-user
counts. `unique_message_users_count` counts distinct non-NULL
`recipient_address` values grouped by `dst_chain_id`
(`select_stats_chains_message_user_counts` in `database.rs`); with
`recipient_address` NULL for every unindexed-source message, that chain's
count was silently deflated (one production chain showed 23 against 13,865
uncounted completed incoming messages). After this fix and a stats rebuild,
operators will see the number jump — that is a correction, not a bug.

---

## `recipient_address` On A Terminal `crosschain_messages` Row Can Never Be Patched Later

**Symptom:** A message's `recipient_address` stays NULL forever even after its
source chain is later added to the bridge config and the real `send` event
(carrying the correct value) is indexed — while `sender_address` and `payload`
*do* get filled in by that same later flush.

**Root cause:** `crosschain_messages_on_conflict` (`message_buffer/persistence.rs`)
does not apply one merge policy to all columns. `dst_chain_id`, `dst_tx_hash`,
and `recipient_address` use `keep_existing_if_terminal`: once `status` is
`completed`/`failed`, the **stored** value is kept unconditionally and the
incoming flush's value for that column is discarded outright — even if the
stored value is NULL and the incoming one is not. `sender_address`, `payload`,
and `src_tx_hash` instead use `prefer_incoming`
(`COALESCE(EXCLUDED.col, stored.col)`), which fills a NULL from a later flush
regardless of terminal status.

**Fix / implication:** a NULL `recipient_address` written on the row's first
terminal flush is **permanent** — there is no later opportunity to backfill
it, unlike `sender_address`/`payload`, which self-heal once a real `send`
arrives even if left NULL initially. This is exactly why the fix above must
populate `recipient_address` at first write for the unindexed-source fallback
path, not lean on "a later `send` will patch it": for this one column, that
assumption is false. If a future change needs `recipient_address` to be
correctable after terminal status, it requires either a different conflict
policy for that column or an explicit reconciliation path — the existing merge
will silently discard the correction.

---

## `pending_messages` Retention for Unconfigured Counterparts Is Load-Bearing, Not a Leak

**Symptom:** `pending_messages` grows and plateaus rather than draining to
zero, even long after catch-up finishes — most visibly for a bridge running
with `process_unknown_chains: true` against subnets it does not configure. A
live run on Avalanche bridge 2 (C-Chain + Numine configured,
`process_unknown_chains: true`) showed roughly 20k cold-tier rows with the
oldest row's timestamp not moving between snapshots.

**Root cause:** Removal from `pending_messages` is gated on **protocol
finality**, not on "has this key been flushed." A message flushed as
`Partial` keeps its buffered cold-tier row — permanently, if its counterpart
chain is unconfigured for the bridge. This is deliberate, not an oversight:

- consolidation reads only the buffered state for a key; it has no path to
  rehydrate a canonical row back into the buffer
- `Consolidate for Message`'s `consolidate()` refuses to proceed for a
  *configured* source chain with no `send` event yet
  (`(None, false) => Ok(None)` in `consolidation.rs`) — it will not fabricate
  a message from a destination-only view when the source chain could still
  produce one
- the retained cold-tier row is exactly what makes adding a chain to a
  bridge later **safe without rescanning the already-indexed chain**, whose
  checkpoint has already moved past the relevant blocks — the buffered row is
  the only place the destination-side information still lives

The asymmetry worth remembering: `SourceData::from_send` only consumes seven
fields off the `send` event, and the canonical row already stores every one
of them. So the retained buffer entry is not needed because information is
missing — it is needed because nothing ever arrives to **trigger**
re-consolidation for that key once its source chain is genuinely never
going to produce a `send`.

**Fix:** Do not treat a non-draining `pending_messages` count as a leak by
itself. Distinguish "still catching up" from "permanently retained": query the
oldest `updated_at`/`created_at` per `(bridge_id, chain_id)` twice, separated
by time, once catch-up has converged — if it has not moved and the message
count matches "one-sided messages whose counterpart chain is unconfigured,"
that is expected. If a chain is later added to a bridge's contracts, do not
attempt to clear or rescan these rows preemptively; the buffered entries pick
up the new configuration correctly once new events restore them into the hot
tier.

To confirm this at runtime against a live database, see
`.memory-bank/runbooks/runtime-verification.md` query G (`pending_messages`
backlog trend).

---

## `bridge_contracts` Is Only A Diagnostic Proxy For Runtime Membership

**Symptom:** A chain that is clearly indexed (or clearly not) for a bridge
disagrees with what `bridge_contracts` shows for that `(bridge_id, chain_id)`
pair.

**Root cause:** `bridge_contracts` is populated by `upsert_bridge_contracts`
at startup, which is intentionally sequenced **after** the stats backfill
pass (see `gotchas.md` / `stats-projection.md` on why `IndexedChains` must
come from in-memory config, never this table). Two effects follow:

- **under-populated during startup backfill:** the table has no rows for the
  current run yet exactly when backfill needs an accurate indexed-chain set
- **permanently over-populated after a chain is removed from a bridge:** no
  code path deletes a `bridge_contracts` row when `bridges.json` drops a
  contract, so the table keeps reporting a chain as configured long after it
  stopped being indexed

**Fix:** Treat `bridge_contracts` as useful for diagnostics and joins, never
as the authoritative "is this chain indexed for this bridge" answer. The
authoritative answer is always `IndexedChains::may_observe`, built once at
startup from the in-memory bridges config
(`interchain-indexer-logic/src/stats/indexed_chains.rs`,
`interchain-indexer-server/src/server.rs`). If a diagnostic query needs the
concrete configured set, prefer `IndexedChains::chain_ids_for(bridge_id)` /
`configured_pairs(...)` semantics as the reference, not a raw
`bridge_contracts` scan.

`.memory-bank/runbooks/runtime-verification.md` spells out this caveat in
full for the queries that join against `bridge_contracts` (D, E).

---

## `just test` / `just test-with-db` Silently Target The Dev Database

**Symptom:** Running `just test` (or `just test-with-db`) appears to pass or
fail against unexpected data, or interferes with a locally running dev
Postgres instance rather than an isolated test database.

**Root cause:** `justfile` computes `DATABASE_URL` from `DB_HOST`/`DB_PORT`
(defaulting to `localhost:5432`, the dev database's usual address) and
`export`s it, which **overrides** any `DATABASE_URL` already set in the
calling shell's environment. `just test-with-db` does start a separate
Postgres on `TEST_DB_PORT` (default `9433`) and reassigns `db-port`/`db-name`
for its own recipe invocation — but any direct `just test` call, or a
shell that already exported a different `DATABASE_URL` expecting it to be
honored, silently loses that value.

**Fix:** For a self-contained run, prefer `just test-with-db`, which manages
its own disposable Postgres end to end. For a targeted single-test run against
a specific database, invoke `cargo test` directly with an explicit
`DATABASE_URL=...` rather than going through `just test`, since the recipe
recomputes and exports the variable regardless of what the shell already set.
`just test-with-db` also runs its underlying `cargo test` as one invocation
across the workspace; a failure aborts that run rather than continuing to
exercise unrelated crates, so a single failing test elsewhere in the workspace
can stop you from seeing results for the crate you actually care about — scope
with `cargo test -p <crate>` when you only need one crate's tests.

---

## Two Pre-Existing `avalanche-e2e` Failures Are Environmental, Not Regressions

**Symptom:** Running the full `avalanche-e2e` suite
(`cargo test --package interchain-indexer-server --features avalanche-e2e --
--ignored --nocapture`) reproducibly fails exactly two tests out of nine,
independent of which commit is checked out.

**Root cause:** Both reproduce identically on `main` and are unrelated to any
in-flight change:

- `test_home_chain_does_not_override_strict_unknown_filter` polls for a
  `chains`/blockchain-ID-mapping row that `blockchain_id_resolver.rs` will
  never persist for its scenario: the resolver only writes a mapping when
  `process_unknown_chains` is set or the chain already exists, and this test
  deliberately configures neither. A test/contract mismatch predating any
  recent branch by several releases, not a network problem (the live
  Avalanche Data API was independently confirmed reachable and correct).
- `test_icm_and_ictt_are_indexed` races a checkpoint write in `log_stream.rs`:
  the realtime cursor is persisted when the catch-up loop exits, and this
  test forks both chains exactly at the event block, so there is no catch-up
  range at all — the checkpoint reports "done" before the realtime fetch of
  that very block has actually been handled. The test's readiness check is
  checkpoint equality, so it queries too early and fails as either
  `message not found` or `status Initiated` depending on scheduling. It is the
  only test in the suite with both sides forked at the event block *and* a
  checkpoint-based wait, which is why the other e2e tests are unaffected.

**Fix:** Do not chase these as regressions when validating a change against
`avalanche-e2e` — confirm they were already failing on `main` first. Fixing
either is a pre-existing-bug task, not part of feature work that happens to
touch nearby code: the resolver mismatch is fixable from either the resolver
or the test's scenario; the checkpoint race needs the catch-up-exit write in
`log_stream.rs` to not precede the realtime fetch it claims to cover, or the
test's readiness check to wait on something other than checkpoint equality.

---

## Token Identity In `stats_asset_tokens` Is The ICTT Contract Address, Not The Wrapped ERC-20

**Symptom:** Two chain-local token rows for the same `stats_asset` show the
identical token address, and it looks like the split-asset detector should
have flagged it but did not.

**Root cause:** This is intentional modelling, not a defect. Avalanche ICTT
transfers key chain-local token identity by the TokenTransferrer contract
address (the Home/Remote bridge contract), not by the wrapped ERC-20 the
contract wraps. When Home and Remote happen to be deployed at the same
address via CREATE2, the two chain-local rows for that asset legitimately
carry the same address — this is one asset observed on two chains, not two
assets that collided. The owner confirmed this modelling is intended
(observed on Avalanche NUMI/WTTC in a live run).

**Fix:** Do not "fix" a same-address pair across two chains in
`stats_asset_tokens` as if it were a bug — the split detector (`stats_asset_id`
per endpoint disagreeing) correctly does not flag it, because it is not a
split. If a genuine split is suspected, verify via the split detector
described in `gotchas.md`, "Stats Asset Mapping Conflicts Merge; Only
Same-Chain Collisions Skip," rather than by eyeballing address equality.

---

## Upgrading Unknown Chains to Proper Bridges

**Symptom:** You have partial messages (unknown source chain) and want to properly index that chain pair.

**Root cause:** Messages from unknown chains are indexed with `init_timestamp = last_update_timestamp` and no `src_tx_hash`. Re-indexing the source chain alone won't "upgrade" existing messages — the upsert would overwrite destination-side data with incomplete source-only data.

**Procedure:**

1. **Create a new bridge** for the chain pair (e.g., A ↔ C) with proper contracts config
2. **Update the original bridge** to stop processing the now-configured pair:
   - set `process_unknown_chains: false`
   - set `home_chain_id: null` (or remove `home_chain_id`) for strict mode
3. **Delete partial messages** from the original bridge (`DELETE FROM crosschain_messages WHERE bridge_id = X AND src_chain_id = C OR dst_chain_id = C`)
4. **Restart** — the new bridge indexes A ↔ C with full data

**Production model:**

```json
[
   {
      "name": "A-B strict bridge",
      "process_unknown_chains": false,
      "home_chain_id": null
   },
   {
      "name": "A-C strict bridge",
      "process_unknown_chains": false,
      "home_chain_id": null
   },
   {
      "name": "Monitoring bridge",
      "process_unknown_chains": true,
      "home_chain_id": 43114
   }
]
```

**Key insight:** Don't try to incrementally upgrade partial messages. Clean delete + fresh re-index is simpler and safer.

---

## Config Env Overrides: Null Replaces, JSON Quoting, Zero-Padded Numbers

Traps in the `INTERCHAIN_INDEXER_CHAINS*` / `INTERCHAIN_INDEXER_BRIDGES*`
env-override layer (`env_merge.rs`, see the README section "Overriding
chains.json / bridges.json via environment"):

**`null` replaces the value, never removes the key.**
`INTERCHAIN_INDEXER_BRIDGES__1__API_URL=null` yields `"api_url": null` in the
merged JSON — the key stays. This is deliberate: fields like `api_url` are
`Option` without `#[serde(default)]`, so key removal would cause
`missing field` errors. `null` for a whole entry
(`INTERCHAIN_INDEXER_CHAINS__137=null`) is a startup error — deletion via env
is not supported.

**A literal string that is valid JSON needs JSON-string quoting.**
Values are parsed as JSON first, falling back to a plain string. `NAME=123`
injects the *number* 123 and fails the typed parse for a string field; use
`NAME='"123"'` for the string `123`. Same for literal `true`/`false`/`null`
strings.

**Zero-padded numbers fall back to strings.**
`06` is not valid JSON (leading zero), so a value of `06` becomes the string
`"06"` and fails the typed parse for numeric fields. Write `6`. (Key *path
segments* like `…__CONTRACTS__100__<addr>__06` are more forgiving — they are
coerced with integer parsing — but don't rely on it.)

Debugging tip: serde errors after merging reference the merged JSON, not the
offending env var. The `applied config env override` info logs printed before
deserialization list every applied var and its JSON path; overrides that
replace an existing value emit an additional info line identifying the
replaced path (`config env override replaced an existing value`). Raw config
values never appear at info level (RPC URLs may embed API keys); enable debug
logging to see the old/new values of replacements.

---

## Filter Params Must Not Reuse Pagination Cursor Field Names

List requests (`GetMessagesRequest`, `GetTransfersRequest`, and their
byTx/byAddress variants) already use `bridge_id` as a **raw pagination
cursor** field (proto field 7), and `GetChainsStatsRequest` uses `chain_id`
(field 8) the same way. A read *filter* with either name would collide with
cursor semantics for `api.use_pagination_token=false` clients.

The unified read-filter vocabulary avoids both by construction:
`home_chain_id`, `counterparty_chain_ids`, `bridge_ids`. Never add request
fields named `bridge_id`/`chain_id` to these messages for non-cursor purposes.

---

## Unknown Query Params Are Silently Ignored by Generated HTTP Routes

`#[actix_prost_macros::serde]` expands to a plain
`#[derive(serde::Serialize, serde::Deserialize)]` **without**
`deny_unknown_fields` (verified in actix-prost-macros 0.3.1), so the
generated HTTP routes drop query parameters that are not declared proto
fields. Consequence: an endpoint cannot reject a filter it does not declare —
clients passing an unsupported filter would silently receive *unfiltered*
data, which for per-frontend slicing means leaking other bridges'/chains'
rows.

Pattern for the read-API filters: declare the field in proto even when the
endpoint cannot honor it yet, and return
`Status::invalid_argument("<param> is not supported by this endpoint yet")`
for non-blank values.

---

## SeaORM `insert_many` Cannot Mix Set and NotSet for the Same Column

**Symptom:** Mock DB seed fails with `null value in column "init_timestamp"
violates not-null constraint` even though some ActiveModels use
`..Default::default()` (expecting the PostgreSQL `DEFAULT now()`).

**Root cause:** In a single `Entity::insert_many([...])` batch, if any model
has `Set(init_timestamp)`, SeaORM includes that column for every row. Models
that left it `NotSet` then insert SQL `NULL` instead of omitting the column
(so the DB default never applies).

**Fix:** Split into separate inserts — one batch that relies on DB defaults,
another that explicitly `Set`s timestamps — or set the column on every model
in the batch.

---

## `abi_decode_validate` Does Not Reject Trailing Bytes

**Symptom:** Decoding a payload with garbage appended after a valid ABI
encoding still succeeds.

**Root cause:** `alloy_sol_types::SolValue::abi_decode_validate` only
type-checks the tokens it reads (correct offsets, correct word count for the
declared type); it does not verify that the input slice was fully consumed.
Non-canonical offsets and trailing bytes both pass silently.

**Fix:** Add an explicit canonicity round trip — re-encode the decoded value
(`decoded.abi_encode()`) and compare it byte-for-byte to the original input.
This is the layer that actually rejects trailing bytes and non-canonical
encodings. See `interchain-indexer-logic/src/indexer/avalanche/ictt_payload.rs`
(`decode_transferrer_message` / `decode_inner`) for the pattern — it matters
here because a false-positive ICTT payload decode would fabricate a bogus
`crosschain_transfers` row from an arbitrary ICM message.

---

## A Checkpoint Certifies Scanning, Not Correctness

**Symptom:** `indexer_checkpoints` shows a chain fully caught up, and
`GET /api/v1/status/indexing` reports `catchup_progress_percent: 100` with
`catchup_scan_complete: true` — while messages from blocks inside that range are
missing from the database.

**Root cause:** A checkpoint records that `eth_getLogs` returned successfully for
a range, plus gaps between known blocks inferred as "scanned but empty". It has
never meant "every covered block was processed without errors". A range that was
fetched and then failed downstream leaves no hot barrier, so a later successful
item moves the frontier straight across it.

**Fix:** Read the two records together. Completeness lives in `indexer_failures`
(the failed-range ledger), not in the cursors. In the API payload,
`failed_blocks != 0` is the only completeness signal — the percentage is the
*scanned* share and reaches 100% with holes still open, by design. Operationally,
alert on `interchain_indexer_oldest_open_hole_age_seconds`: the retry pass is the
only recovery path, so a hole that stops draining is invisible otherwise.

**The converse does not hold.** `failed_blocks == 0` means "nothing was
recorded", not "nothing was lost". A failure only becomes a row if it reaches the
driver as a `BatchError`, so an error a handler swallows is invisible — and worse,
a replay covering that range reads as success and `resolve`s an existing hole.
Malformed input is skipped as data quality on purpose (a log without
`transaction_hash`, an event without `topic0`, a failed token-enrichment decode),
and `resolve` runs before the mutation is durable. When adding an indexer or an
event handler, propagating the failure is what buys you the guarantee; nothing
else does. See ADR-005 and
`.memory-bank/research/indexing-gaps-retries-and-checkpoint-safety.md`.

---

## The Failure Ledger's Healthy Path Is DB-Free Only Because One Process Owns A Bridge

**Symptom:** Running two replicas against the same database appears to work, then
produces overlapping `indexer_failures` rows, over-reported `failed_blocks`, and
holes that resolve on one replica while another still believes they are open.

**Root cause:** `FailureLedger` keeps an in-memory set of `(bridge_id, chain_id)`
pairs known to have open holes, so a successful batch on a healthy chain performs
**zero** database statements. That cache may be stale-*true* (one redundant
query, harmless) but must never be stale-*false* — which holds only while a single
process indexes a given bridge. `failed_blocks` also sums row widths directly,
which is exact only because rows for a pair are disjoint and non-adjacent, an
invariant maintained by merge-on-write within one writer.

**Fix:** Keep one replica per bridge. The assumption was already implicit in
checkpointing (`LEAST`/`GREATEST` upserts tolerate concurrency but do not make it
correct); the ledger simply gives it a second consumer. If multi-writer ever
becomes real, the ledger needs a database-level non-overlap constraint
(`EXCLUDE USING gist`) before the cache can be trusted.

---

## The AMB Scan Floor Is The `amb_proxy` Contract's `started_at_block`

**Symptom:** Lowering `omnibridge_mediator`'s `started_at_block` in
`config/omnibridge/bridges.json` changes nothing — no earlier history is scanned,
and the progress endpoint's denominator does not move.

**Root cause:** AMB declares two contracts per `(bridge, chain)`, but one log
stream covers both addresses, and its floor comes from the `amb_proxy` entries
alone. In the shipped config the mediator's value sits ~7.4M blocks below the
proxy's on chain 1, so taking `min` across a chain's contracts would understate
the scan floor by that much.

**Fix:** Change the `amb_proxy` value to reach earlier history. Do not add a
second place that derives a floor: `ChainPlan`/`plan_bridge`
(`interchain-indexer-server/src/indexers.rs`) is the one selection rule, and the
progress denominator, the running indexer's `genesis_block` and the startup
floor reconciliation all read it. The rule is `min` over the contracts of the
kind that drives the scan — `amb_proxy` for AMB, all of them otherwise — and
`min` rather than "the first one" because several versions of the same kind is
how a contract upgrade is expressed. An AMB chain with no `amb_proxy` yields no
floor at all rather than falling back to the mediator's: it cannot be indexed,
and a plausible number for a scan that never runs is worse than none.

A mediator floor *above* the proxy floor is legal — AMB-only operation indexes
messages without transfers — and both directions of divergence are warned about
once at startup by `log_amb_floor_divergence`.

---

## A Replay Can Only Recover What The Replay Can Still Find

**Symptom:** A recorded `indexer_failures` row disappears after a retry pass
that "succeeded", but the data it covered is still missing.

**Root cause:** The ledger records *block ranges*, and a replay re-fetches those
blocks and reprocesses them. Anything the failed attempt consumed from memory —
an entry taken out of a correlation queue, a lookup removed, a channel drained —
is not restored by re-fetching the blocks. The replay then finds nothing to do,
returns `Ok`, and `resolve` deletes the row. The concrete instance: AMB's
message-hash queue used to `remove` the pending entry before running the
fallible applies, so a mid-drain failure lost the remainder while the block was
correctly recorded. Both halves looked right in isolation.

**Fix:** Consume in-memory state **last**. Work on a clone, and remove the
original only after every fallible step has succeeded. This makes the operation
at-least-once rather than exactly-once, which is already the regime for any
replayed range. When reviewing a new adapter, the question is not "is the
failure recorded" but "will a replay of those blocks reconstruct everything the
failed attempt consumed".

---
