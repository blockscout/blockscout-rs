# Gotchas

Non-obvious traps and their solutions.

## Message Finality is Complex

**Symptom:** Messages stuck in "Initiated" status despite execution events arriving.

**Root cause:** A message is NOT final if:
- Execution failed (can be retried via `retryMessageExecution()`)
- ICTT transfer incomplete (waiting for destination-side events)

**Fix:** Finality requires: execution succeeded AND ICTT transfer complete (if applicable). Check `consolidation.rs` for the logic.

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
