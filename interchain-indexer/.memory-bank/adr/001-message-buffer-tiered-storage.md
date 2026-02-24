# ADR-001: Message Buffer Tiered Storage

**Status:** Accepted

**Date:** 2026-01

## Context

Cross-chain messages arrive as multiple events across different transactions and even different chains. A single message might require:
1. SendCrossChainMessage event on source chain
2. ReceiveCrossChainMessage event on destination chain
3. MessageExecuted event on destination chain
4. Token transfer events on both chains (for ICTT)

These events arrive asynchronously and out of order. We need to buffer partial messages until all required events arrive to determine finality.

**Forces:**
- High throughput: thousands of events per second across multiple chains
- Low latency: finalized messages should be available quickly
- Durability: partial messages must survive restarts
- Memory efficiency: can't keep everything in memory indefinitely

## Decision

Implement a tiered storage system for the message buffer:

1. **Hot tier (in-memory):** `DashMap` for fast concurrent access
2. **Cold tier (PostgreSQL):** `pending_messages` table for durability
3. **Eviction:** TTL-based with configurable hot TTL (default 10s)
4. **Restoration:** Load from cold tier on cache miss

Key implementation details:
- Entry versioning for cursor tracking
- Maintenance task runs on configurable interval (default 500ms)
- Consolidation check on each maintenance pass
- Metrics for buffer hits/misses/evictions

## References

- `interchain-indexer-logic/src/message_buffer/` - Implementation
- `interchain-indexer-logic/src/message_buffer/buffer.rs` - Core buffer logic
- `interchain-indexer-logic/src/message_buffer/persistence.rs` - Cold tier operations
