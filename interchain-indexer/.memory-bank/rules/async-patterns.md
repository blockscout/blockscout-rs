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

Prefer stabilized Rust async traits for statically-dispatched traits.
Use `#[async_trait]` only when dynamic dispatch (`dyn Trait`) is required.

```rust
pub trait CrosschainIndexer: Send + Sync {
    async fn start(&self) -> Result<(), Error>;
}
```

If the trait must be used as a trait object (`Box<dyn CrosschainIndexer>`), use
`#[async_trait]` intentionally and document why dynamic dispatch is needed.

## Shared State

Use `Arc<RwLock<T>>` for shared mutable state snapshots across tasks.
Use channels when ownership transfer or event
fan-out is the natural model.

```rust
// Good
pub struct Service {
    state: Arc<RwLock<State>>,
}

// Access
let state = self.state.read().await;
```

## Per-Key Locking

Use `DashMap` only when keyed-concurrency contention is real and measured.
For simpler flows, prefer explicit lock-based maps for clearer locking behavior.
When using `DashMap`, follow its locking caveats carefully to avoid deadlocks.

```rust
// Good: lock-free concurrent hashmap
let buffer: DashMap<Key, Entry> = DashMap::new();

// Atomic get-or-insert
buffer.entry(key).or_insert_with(|| Entry::new());
```

Safety notes:
- Do not hold `DashMap` guards across `.await` points.
- Avoid nested access patterns that can re-enter the same shard lock.

## Sync Locks

Prefer `std::sync::{Mutex, RwLock}` for sync locks.
Lock poisoning is useful as a signal after panic-related corruption.
Use `parking_lot` only with explicit justification (for example, measured hot
paths where std locks are a bottleneck).

```rust
use std::sync::RwLock;

// For non-async contexts
let guard = self.cache.read().expect("lock poisoned");
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
