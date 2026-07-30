# Architecture

## High-Level Data Flow

```text
Blockchain RPC
    ↓
LogStream (catchup + realtime modes)
    ↓ (Filter logs, batch by block)
Indexer Event Handlers (parse logs → typed events)
    ↓ (Group by transaction)
MessageBuffer.alter()
    ├→ Get-or-create buffer entry
    ├→ Mutate entry (add events)
    └→ Record cursor for safe advancement
    ↓
Maintenance Task
    ├→ Consolidate (check finality)
    ├→ Flush to PostgreSQL
    └→ Evict expired entries
    ↓
InterchainDatabase (upserts)
    ├→ crosschain_messages
    ├→ crosschain_transfers
    └→ pending_messages
```

## Core Abstractions

### CrosschainIndexer Trait

Location: `interchain-indexer-logic/src/indexer/crosschain_indexer.rs`

Plugin interface for bridge indexers.

```rust
pub trait CrosschainIndexer: Send + Sync {
    fn name(&self) -> String;
    fn description(&self) -> String;
    async fn start(&self) -> Result<(), Error>;
    async fn stop(&self);
    fn get_state(&self) -> CrosschainIndexerState;
    fn get_status(&self) -> CrosschainIndexerStatus;
}
```

States: `Idle` → `Running` → `Idle` or `Failed(String)`

### MessageBuffer

Location: `interchain-indexer-logic/src/message_buffer/`

Tiered storage system for assembling cross-chain messages from multiple events:

- **Hot tier:** In-memory `DashMap` for fast access
- **Cold tier:** PostgreSQL for persistence
- **Features:** Entry versioning, TTL-based eviction, cursor tracking

### LogStream

Location: `interchain-indexer-logic/src/log_stream.rs`

Bidirectional blockchain log streaming:

- **Catchup mode:** Finite stream of historical blocks
- **Realtime mode:** Continuous stream of new blocks
- **Checkpointing:** Safe restart from last processed block

### Consolidate Trait

Location: `interchain-indexer-logic/src/message_buffer/types.rs`

Determines when a buffered message is ready for database persistence:

```rust
pub trait Consolidate: Clone + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de> {
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>>;
}
```

Three outcomes, not two: `Ok(None)` (not yet consolidatable), `Ok(Some(.. is_final:
false ..))` (partial — flushed but kept in the buffer), `Ok(Some(.. is_final: true
..))` (complete — flushed and evicted). "Ready for finality" is not simply "all
expected events received" for every protocol — see
`.memory-bank/research/message-lifecycle.md` for the full contract and
`interchain-indexer-logic/src/indexer/avalanche/consolidation.rs` for how
Avalanche's finality rule (execution success **and** ICTT completeness, where
completeness now also accounts for hops that never produce a destination
credit) fulfills it.

### `IndexedChains` (Stats Eligibility)

Location: `interchain-indexer-logic/src/stats/indexed_chains.rs`

The stats layer's single observability-horizon predicate: which chains a
bridge indexes, i.e. where its events can be observed at all. Answers "can
this evidence still arrive?" for both stats projection eligibility and the
read-side unindexed-chain filter, from one in-memory-config-derived set —
never from the `bridge_contracts` table. See
`.memory-bank/adr/004-stats-observability-horizon-and-asset-union-find.md`
and `.memory-bank/research/stats-subsystem.md`.

## Global Services

### ChainInfoService

Location: `interchain-indexer-logic/src/chain_info/`

Cached chain metadata (name, icon, explorer URLs). Falls back to "Unknown" for unconfigured chains.

### TokenInfoService

Location: `interchain-indexer-logic/src/token_info/`

Resolves token metadata (symbol, decimals, icon) via on-chain calls and Blockscout API. Uses per-key locking and background fetching to avoid duplicate requests.

<!-- TODO: Replace ASCII schema with more descriptive diagram from Notion -->
## Database Schema

```text
chains (id, name, icon, explorer, custom_routes)
    ↑
bridges (bridge_id, name, indexer, status)
    ↑
bridge_contracts (bridge_id, chain_id, address, kind)
    ↑
crosschain_messages (id, bridge_id, src_chain, dst_chain, status, ...)
    ↑
crosschain_transfers (message_id, bridge_id, token_address, amount, ...)

pending_messages (intermediate state before finality)
indexer_checkpoints (chain_id, bridge_id, block_number)
indexer_failures (error tracking)
tokens (cached token metadata)

stats_messages (bridge_id, src_chain_id, dst_chain_id, messages_count)
stats_messages_days (date, bridge_id, src_chain_id, dst_chain_id, messages_count)
stats_assets / stats_asset_tokens (logical bridged-token asset ↔ chain-local tokens, union-find merged)
stats_asset_edges (stats_asset_id, bridge_id, src_chain_id, dst_chain_id, cumulative_amount)
stats_chains (chain_id, unique_transfer_users_count, unique_message_users_count — periodic snapshot)
```

Stats tables are projections from `crosschain_messages`/`crosschain_transfers`,
not primary ingestion tables — see `.memory-bank/research/stats-projection.md`
and `.memory-bank/research/stats-subsystem.md`.

## Indexer Implementations

### AvalancheIndexer

Primary implementation for Avalanche ecosystem:

- **Protocols:** Teleporter (ICM) cross-chain messaging + ICTT token transfers
- **Features:**
  - Multi-chain parallel log streaming
  - Blockchain ID resolution (native → EVM)
  - Transaction-grouped event processing
  - Incremental message state building
  - Chain-based event filtering
