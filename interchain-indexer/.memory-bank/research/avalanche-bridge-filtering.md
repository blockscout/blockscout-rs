# Avalanche Bridge Filtering

## Scope

How the Avalanche indexer decides which Teleporter events to store: the
interaction between `process_unknown_chains`, `home_chain_id`,
configured-chain checks, and blockchain ID resolution. Covers the filtering
gate from log ingestion through to the `buffer.alter()` boundary.

Does not cover:

- Blockchain ID resolution internals (see `avalanche-blockchain-id-resolution.md`)
- Message consolidation or finality rules (see `message-lifecycle.md`)
- Buffer, maintenance, or persistence internals (see `message-lifecycle.md`)
- Operational procedures for upgrading unknown chains (see `gotchas.md`,
  "Upgrading Unknown Chains to Proper Bridges")

## Short Answer

Filtering is a pre-buffer storage gate that determines which ingested
Teleporter events produce stored records. It operates on two conceptually
distinct chain sets:

- **Indexed set** — chains we actively stream logs from. Defined entirely by
  config files (`chains.json` RPCs + `bridges.json` contracts). Frozen at
  indexer startup. This is `chain_ids` in the code.
- **Exposed set** — chains the service knows about and serves via API. Lives
  in the `chains` table. Seeded from config at startup, then expanded at
  runtime when the resolver discovers new chains.

The filtering flags (`process_unknown_chains`, `home_chain_id`) control how
events ingested from the indexed set expand the exposed set. They do not
change what we index *from* — the log source is always and only the indexed
set. They control what we *store*.

## Why This Matters

Bridge filtering is the primary policy mechanism for controlling which
cross-chain messages are persisted. Misconfiguration silently drops events
(trace-level logs only). Understanding the two-set model and the two-stage
filter is essential for configuring bridges correctly and diagnosing missing
messages.

## Source-of-Truth Files

- `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
  - `should_process_message()` (lines 435–452)
  - `chain_ids` construction (line 205)
  - per-handler filtering call sites
- `interchain-indexer-server/src/config.rs`
  - `BridgeConfig`: `process_unknown_chains`, `home_chain_id`
- `interchain-indexer-server/src/indexers.rs`
  - `build_avalanche_chain_configs()`: indexed-set assembly
- `interchain-indexer-server/src/server.rs`
  - startup wiring: config loading, provider creation, indexer spawning
- `interchain-indexer-logic/src/indexer/avalanche/consolidation.rs`
  - `source_chain_is_unknown` effect on consolidation
- `config/avalanche/bridges.json`
- `config/avalanche/chains.json`

## Key Types / Tables / Contracts

- `BridgeConfig.process_unknown_chains` — `bool`, default `false`
- `BridgeConfig.home_chain_id` — `Option<ChainId>`, default `None`
- `chain_ids: HashSet<i64>` — the indexed set, built per-bridge at startup
- `should_process_message()` — pure function, two-stage filter
- `LogHandleContext` / `BatchProcessContext` — carry filter params through
  handler call chain
- `Message.source_chain_is_unknown` — `bool`, set post-filter inside buffer
  mutation

## Step-by-Step Flow

### 1. Indexed set construction

A chain enters the indexed set for a bridge when **all** of these hold:

1. It has a contract entry in `bridges.json` for that bridge
2. It has a chain config in `chains.json`
3. At least one RPC provider in `chains.json` is enabled
4. Provider construction succeeds

`build_avalanche_chain_configs()` in `indexers.rs` applies conditions 2–4.
The resulting `Vec<AvalancheChainConfig>` is passed to `AvalancheIndexer::new()`.

At `AvalancheIndexer::run()`, the indexed set is frozen:

```rust
let chain_ids: HashSet<i64> = chains.iter().map(|c| c.chain_id).collect();
```

This set is **never reloaded from the database**. After a restart it is
rebuilt from the same config files. Chains discovered by the resolver and
persisted into the `chains` table do not affect `chain_ids`.

This is intentional. Dynamic expansion would require mid-run changes to log
stream subscriptions, checkpoint state, and provider pools. The safe path is
config change + restart.

### 2. Blockchain ID resolution (hard prerequisite)

Teleporter events carry 32-byte Avalanche native blockchain IDs, not EVM
chain IDs. The `chain_ids` set contains EVM chain IDs. Therefore resolution
**must** happen before filtering — the filter input does not exist until the
native ID is translated.

Each handler resolves the peer chain's native blockchain ID via
`BlockchainIdResolver` before calling `should_process_message()`. See
`avalanche-blockchain-id-resolution.md` for resolution internals.

Resolver failure (Data API down, invalid response) prevents the filter from
being evaluated and the event is skipped — resolution failure acts as an
implicit third filter.

### 3. Two-stage filter: `should_process_message()`

The function applies two sequential checks:

**Stage 1 — Chain config filter:**

```
passes = (src_configured AND dst_configured)
      OR ((src_configured OR dst_configured) AND process_unknown_chains)
```

Both-unknown is **always** rejected, even with `process_unknown_chains = true`.

**Stage 2 — Home chain filter** (only when `home_chain_id` is set):

```
passes = (source == home_chain) OR (dest == home_chain)
```

Both stages must pass.

`home_chain_id` validation: if set, it must name one of the indexed chains.
The constructor rejects invalid values at startup. If omitted (default
`None`), no home-chain filter is applied and the indexer starts normally.

### 4. Per-handler filtering

| Handler | Resolves | source_chain_id | dest_chain_id |
|---------|----------|-----------------|---------------|
| `SendCrossChainMessage` | `destinationBlockchainID` | `ctx.chain_id` (emitting chain) | resolved destination |
| `ReceiveCrossChainMessage` | `sourceBlockchainID` | resolved source | `ctx.chain_id` (emitting chain) |
| `MessageExecuted` | `sourceBlockchainID` | resolved source | `ctx.chain_id` (emitting chain) |
| `MessageExecutionFailed` | `sourceBlockchainID` | resolved source | `ctx.chain_id` (emitting chain) |

All four handlers follow the same pattern: decode → resolve → filter →
`buffer.alter()`. If the filter rejects, the handler returns `Ok(())`
immediately. Filtered events produce trace-level logs with the message
"filtered by bridge chain policy" and full diagnostic context.

### 5. Pre-buffer gate

Filtered events **never** enter the buffer. No `buffer.alter()` is called,
no `touched_blocks` are recorded, no cursor advancement is contributed.

### 6. Post-filter: `source_chain_is_unknown` flag

When a message passes filtering with an unconfigured source chain
(`process_unknown_chains = true`), handlers `ReceiveCrossChainMessage`,
`MessageExecuted`, and `MessageExecutionFailed` set
`msg.source_chain_is_unknown = true` inside the `buffer.alter()` closure.

This flag causes two levels of degradation during consolidation:

1. **Messaging degradation** — consolidation falls back to
   `SourceData::from_receive()` or `SourceData::from_execution()` instead of
   `SourceData::from_send()`. The resulting message has no `src_tx_hash`, no
   `sender_address`, no `payload`, and `init_timestamp` equals the
   destination-side block timestamp instead of the source-side timestamp.

2. **Complete ICTT transfer loss** — transfer building at
   `consolidation.rs:189–195` requires both `self.send` and `self.transfer`
   to be present. Without a send event, `self.send` stays `None`, so the
   consolidated message always gets `transfers: Vec::new()` — **zero
   `crosschain_transfers` rows**. This is not a simplification; it reflects a
   hard constraint: source-side ICTT logs (`TokensSent`, `TokensRouted`,
   etc.) carry the sender address, amount, source token address, and
   destination token address needed to build a transfer record. Destination-
   side events alone do not carry enough data to reconstruct this.
   Implementing destination-only ICTT reconstruction is possible but currently
   unimplemented due to complexity.

The flag tells consolidation: "we will never get a send event for this
message, so stop waiting and consolidate with what we have."

## Full Truth Table

Configured chains = {1, 2, 3}. Verified against `rstest` cases in
`mod.rs`.

| src | dst | process_unknown | home_chain | result |
|-----|-----|-----------------|------------|--------|
| configured | configured | false | None | **pass** |
| configured | unknown | false | None | reject |
| unknown | configured | false | None | reject |
| unknown | unknown | false | None | reject |
| configured(home) | configured | false | Some(home) | **pass** |
| configured | configured(not home) | false | Some(home) | reject |
| configured(home) | unknown | false | Some(home) | reject (stage 1) |
| unknown | configured(home) | false | Some(home) | reject (stage 1) |
| configured | unknown | true | None | **pass** |
| unknown | configured | true | None | **pass** |
| unknown | unknown | true | None | reject |
| configured(home) | configured | true | Some(home) | **pass** |
| configured(home) | unknown | true | Some(home) | **pass** |
| unknown | configured(home) | true | Some(home) | **pass** |
| configured | configured(not home) | true | Some(home) | reject (stage 2) |
| configured(not home) | unknown | true | Some(home) | reject (stage 2) |
| unknown | unknown | true | Some(home) | reject |

## Invariants

- The indexed set is frozen at startup and rebuilt only from config files on
  restart. It is never reloaded from the database.
- `home_chain_id`, when set, must name an indexed chain (validated at
  construction).
- Both-unknown messages are always rejected regardless of flags.
- Blockchain ID resolution is a hard prerequisite for filtering, not an
  optimization that could be reordered.
- Filtered events never enter the buffer, never contribute to checkpoints,
  and never reach consolidation or persistence.
- The resolver and `chains` table are global across bridges; the filtering
  decision is per-bridge.

## Failure Modes / Observability

- Filtered events: trace-level logs with "filtered by bridge chain policy"
  and full context (message ID, chain IDs, `process_unknown_chains`,
  `home_chain`).
- Resolver failure: prevents filter evaluation entirely; event is skipped
  (see `avalanche-blockchain-id-resolution.md`).
- Checkpoint stall: when all events for a chain/bridge pair are perpetually
  filtered, the checkpoint for that pair never advances. No livelock during
  normal runtime (LogStream progresses in memory), but on restart the indexer
  replays from the stale checkpoint. See `gotchas.md`, "Checkpoint Stall When
  All Events Are Perpetually Filtered".
- Silent misconfiguration: a chain missing from `chains.json` or with no
  enabled RPCs is silently excluded from the indexed set. Check startup logs
  for "No enabled RPC providers found for chain" or "Chain configuration
  missing for Avalanche indexer".

## Edge Cases / Gotchas

- Resolution cost for filtered messages is unavoidable: the resolver must
  translate the native blockchain ID before the filter can evaluate. This
  cost is amortized by the resolver cache.
- Cross-bridge resolver persistence: if bridge A (permissive) discovers and
  persists a chain, bridge B (strict) benefits from the cached resolution
  but still rejects the message per its own policy. See `gotchas.md`,
  "Cross-Bridge Resolver Persistence Leaks".
- Unknown-source messages are stored as messaging-only records with no
  `crosschain_transfers` rows. The ICTT layer is invisible for these
  messages.
- Upgrading unknown chains to proper bridges requires a clean delete + fresh
  re-index, not incremental patching. See `gotchas.md`, "Upgrading Unknown
  Chains to Proper Bridges".

## Change Triggers

Update this note when:

- `should_process_message()` logic or signature changes
- `chain_ids` construction or provenance changes
- new filtering flags are added to `BridgeConfig`
- the relationship between indexed set and exposed set changes
- new event handlers are added with different filtering behavior
- `source_chain_is_unknown` semantics or consolidation fallback changes
- destination-only ICTT reconstruction is implemented

## Open Questions

None currently. All questions raised during research were resolved through
discussion.
