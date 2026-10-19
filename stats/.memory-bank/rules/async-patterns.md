---
paths:
  - "stats/src/data_source/**"
  - "stats/src/update_group.rs"
  - "stats-server/src/**"
globs:
  - "stats/src/data_source/**"
  - "stats/src/update_group.rs"
  - "stats-server/src/**"
---

# Async Patterns Rules

Rules for async code in the `DataSource`/update-group framework and the
`stats-server` runtime.

## Trait Methods: RPITIT by default, `#[async_trait]` only for trait objects

`DataSource` (`stats/src/data_source/source.rs`) is the core trait every
chart/dependency implements, and it is **not** object-safe — it's used purely
through static generics (`MainDependencies`, `ResolutionDependencies`
associated types). Its "to be implemented" methods use stable
return-position-`impl Trait` in traits (RPITIT), not `#[async_trait]`:

```rust
fn init_itself(
    db: &DatabaseConnection,
    init_time: &chrono::DateTime<Utc>,
) -> impl Future<Output = Result<(), DbErr>> + Send;
```

Its recursive dispatch methods (`init_recursively`, `update_recursively`)
can't use RPITIT because they call themselves through associated types,
which the compiler can't resolve into a finite return type — those are
written by hand as `-> BoxFuture<'a, ...>` with an explicit
`async move { ... }.boxed()` body:

```rust
fn init_recursively<'a>(
    db: &'a DatabaseConnection,
    init_time: &'a chrono::DateTime<Utc>,
) -> BoxFuture<'a, Result<(), DbErr>> {
    async move {
        Self::MainDependencies::init_recursively(db, init_time).await?;
        Self::ResolutionDependencies::init_recursively(db, init_time).await?;
        Self::init_itself(db, init_time).await
    }
    .boxed()
    // had to juggle with boxed futures because of recursive async calls
}
```

`#[async_trait]` is reserved for traits that genuinely need dynamic dispatch
as a trait object — e.g. `UpdateGroup` (`stats/src/update_group.rs`), used as
`Arc<dyn UpdateGroup + Send + Sync>` (`ArcUpdateGroup`) so `RuntimeSetup` can
hold a heterogeneous collection of groups. Don't reach for `#[async_trait]`
on a new static-dispatch trait just because it's convenient — follow
`DataSource`'s RPITIT pattern instead, and only fall back to `BoxFuture` when
the method is genuinely recursive through associated types.

## Shared State

- Per-update-group synchronization: `tokio::sync::Mutex<()>` per chart name,
  held only long enough to guard the update itself
  (`SyncUpdateGroup::dependencies_mutexes`, `stats/src/update_group.rs`).
- Per-update-cycle memoization: `UpdateCache` (`stats/src/data_source/types.rs`)
  wraps `Arc<Mutex<HashMap<String, CacheValue>>>` — cheap to clone, freshly
  constructed once per group update, and deliberately has no invalidation
  logic (a new one is created and dropped every update).
- There is no `DashMap` (or other sharded-lock map) usage anywhere in this
  workspace — don't introduce one without a measured need; the existing
  concurrency needs are met by `tokio::sync::Mutex` plus lock ordering.

## Lock Ordering (Deadlock-Free Group Updates)

`SyncUpdateGroup` (`stats/src/update_group.rs`) locks its dependency mutexes
in **lexicographic order of chart/mutex name** (iterating a `BTreeMap`), not
in an arbitrary or insertion order:

```rust
async fn lock_in_order(&self, mut to_lock: HashSet<String>) -> Vec<MutexGuard<'_, ()>> {
    let mut guards = vec![];
    for (name, mutex) in self.dependencies_mutexes.iter() { // BTreeMap: sorted by key
        if to_lock.remove(name) {
            let guard = /* try_lock, else wait */;
            guards.push(guard);
        }
    }
    guards
}
```

This is what makes concurrent group updates deadlock-free — but **only** as
long as every group goes through `SyncUpdateGroup` and every group shares the
exact same mutex map (built once in `stats-server/src/runtime_setup.rs`).
Never construct a bespoke ad hoc mutex acquisition order for a new group; use
`SyncUpdateGroup::new` with the shared mutex map.

## Task Spawning

`stats-server/src/server.rs` tracks every long-lived task through a
`tokio_util::task::TaskTracker` + `tokio::task::JoinSet`
(`spawn_and_track`), and joins on `futures.join_next()` to detect the first
task exiting (success or panic) before triggering `on_termination` and
`futures.abort_all()`. Follow this pattern for new long-lived background
tasks in `stats-server` rather than a bare `tokio::spawn` with no tracked
handle.

## Streams / Fan-out

`stats-server/src/read_service.rs` uses `futures::stream::FuturesOrdered` +
`StreamExt` for concurrently resolving multiple chart queries while
preserving response order, and plain `tokio::join!` when the exact set of
futures is fixed and small (see its multiple `join!(...)` call sites). Prefer
`join!` for a fixed, small tuple of independent futures; reach for
`FuturesOrdered`/`FuturesUnordered` when the set size is dynamic.

## Graceful Shutdown

`GracefulShutdownHandler`/`shutdown_token` (from
`blockscout_service_launcher::launcher`) is threaded through `stats()` in
`server.rs`; `on_termination` closes both DB connections
(`db.close_by_ref()`, `indexer.close_by_ref()`, optionally
`cctx_indexer.close_by_ref()`) before cancelling the shutdown token and
aborting the remaining task set. Any new long-lived resource (a connection,
a background loop) added to `stats()` should be closed/cancelled from
`on_termination` alongside the existing ones, not left to `Drop`.
