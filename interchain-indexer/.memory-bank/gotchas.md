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
