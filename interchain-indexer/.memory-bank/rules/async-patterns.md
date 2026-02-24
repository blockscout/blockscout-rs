---
paths:
  - "interchain-indexer-logic/src/indexer/**"
  - "interchain-indexer-logic/src/message_buffer/**"
  - "interchain-indexer-logic/src/log_stream.rs"
  - "interchain-indexer-server/src/**"
globs:
  - "interchain-indexer-logic/src/indexer/**"
  - "interchain-indexer-logic/src/message_buffer/**"
  - "interchain-indexer-logic/src/log_stream.rs"
  - "interchain-indexer-server/src/**"
---

# Async Patterns Rules

Rules for async code in indexers, message buffer, and server components.

## Trait Methods

Use `#[async_trait]` for async methods in traits:

```rust
use async_trait::async_trait;

#[async_trait]
pub trait CrosschainIndexer: Send + Sync {
    async fn start(&self) -> Result<(), Error>;
}
```

## Shared State

Use `Arc<RwLock<T>>` for shared mutable state across tasks:

```rust
// Good
pub struct Service {
    state: Arc<RwLock<State>>,
}

// Access
let state = self.state.read().await;
```

## Per-Key Locking

Use `DashMap` for concurrent access to keyed data:

```rust
// Good: lock-free concurrent hashmap
let buffer: DashMap<Key, Entry> = DashMap::new();

// Atomic get-or-insert
buffer.entry(key).or_insert_with(|| Entry::new());
```

## Sync Locks

Use `parking_lot` for sync locks (no poisoning):

```rust
use parking_lot::RwLock;

// For non-async contexts
let guard = self.cache.read();
```

## Stream Processing

Use `StreamExt` combinators for async streams:

```rust
use futures::StreamExt;

while let Some(batch) = stream.next().await {
    process(batch).await?;
}
```

## Task Spawning

Always handle task join errors:

```rust
let handle = tokio::spawn(async move {
    // task work
});

// Later
match handle.await {
    Ok(result) => { /* use result */ }
    Err(e) => tracing::error!(err = ?e, "task panicked"),
}
```

For long-lived tasks:
- Store `JoinHandle`s on the owning service/indexer struct.
- Do not fire-and-forget loops that outlive `stop()`.
- If task startup can fail, log and continue per indexer when possible instead of aborting all workers.

## Start/Stop Invariants

- `start()` should be idempotent and race-safe.
- `stop()` should be idempotent and safe to call after partial start failures.
- Use explicit started/stopping guards (for example, atomic state transitions) to avoid duplicate starts.
- Ensure maintenance/background tasks are stopped before returning from `stop()`.

## Graceful Shutdown

Implement cleanup guards for stateful tasks:

```rust
struct CleanupGuard<'a> {
    state: &'a Arc<RwLock<State>>,
}

impl Drop for CleanupGuard<'_> {
    fn drop(&mut self) {
        // Reset state even on panic
    }
}
```

Shutdown checklist:
- cleanup executes on all exits (normal return, error, cancellation)
- no leaked per-key locks or entries after early returns
- pending maintenance loops are terminated deterministically
