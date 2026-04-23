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
pub trait Consolidate {
    fn consolidate(&self) -> Option<ConsolidatedMessage>;
}
```

Returns `Some` when message has reached finality (all expected events received).

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
chains (chain_id, name, native_id, explorer_url)
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
```

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
