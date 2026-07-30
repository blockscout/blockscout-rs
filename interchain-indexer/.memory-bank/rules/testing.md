# Testing Rules

Rules for writing and organizing tests. See also `../../RUST_CODE_STYLE_GUIDE.md` for monorepo-wide conventions.

## Test Attributes

```rust
// Async tests
#[tokio::test]

// Database tests: keep `#[tokio::test]` for async DB calls and `#[ignore]` to run them intentionally (typically via `just test-with-db`)
#[tokio::test]
#[ignore]

// Parametrized tests
#[rstest]
#[case(input1, expected1)]
#[case(input2, expected2)]
```

## Feature-Flagged Tests (E2E)

End-to-end tests are gated behind feature flags to avoid running them in CI by default:

```toml
# In Cargo.toml
[features]
avalanche-e2e = []
```

```rust
// In tests/avalanche_e2e.rs
#![cfg(feature = "avalanche-e2e")]
```

Run with:
```bash
cargo test --package interchain-indexer-server --features avalanche-e2e -- --ignored --nocapture
```

Use feature flags for tests that:
- Require external network access
- Need forked blockchain nodes (Anvil)
- Have long execution times
- Depend on external services

## Database Tests

Use `TestDbGuard` for isolated database tests:

```rust
use blockscout_service_launcher::test_database::TestDbGuard;

#[tokio::test]
#[ignore = "needs database"]
async fn test_with_database() {
    let db = TestDbGuard::new::<Migrator>("test_name").await;
    // Test code using db.client()
}
```

## Test Organization

- Unit tests: `#[cfg(test)] mod tests { }` at end of module
- Integration tests: `tests/` directory in crate root
- Helpers: `tests/helpers/mod.rs`

## Mock Data

Use fixtures from `interchain-indexer-logic/src/test_utils/mock_db.rs`:

```rust
use crate::test_utils::mock_db::fill_mock_interchain_database;

let db = init_db().await;
fill_mock_interchain_database(&db).await;
```

## Running Tests

```bash
# Runs database-backed tests with a temporary Postgres instance
just test-with-db [test_name]

# Runs a specific non-ignored test
just test [test_name]
```

`just test` runs `cargo test -- --include-ignored`. Use `just test-with-db` when you need to run ignored database-backed tests. For a single non-database, non-ignored test, `just test [test_name]` is acceptable. Avoid running bare `just test` unless `DATABASE_URL` points to a running Postgres instance for the ignored database-backed tests.


## Test Naming

- Describe what is being tested and expected outcome
- Format: `test_<function>_<scenario>_<expected>`
- Example: `test_consolidate_incomplete_message_returns_none`

## Assertions

- Prefer specific assertions over generic `assert!`
- Use `assert_eq!` with descriptive messages
- For complex assertions, use `pretty_assertions` crate

## Never Assert A Before/After Delta On A Process-Wide Metric

Do not write a test that reads a `lazy_static` Prometheus counter/gauge
before an operation, runs the operation, reads it again, and asserts an exact
delta:

```rust
// Bad: not test-isolated under `cargo test`'s default thread-per-test
// parallelism. Any other test in the same binary that increments the same
// counter can execute inside this before/after window.
let before = SOME_COUNTER.get();
do_the_thing().await;
let after = SOME_COUNTER.get();
assert_eq!(after - before, 1);
```

This is wrong by construction, not merely flaky: the counter is shared by
every test in the binary, so a delta assertion has no isolation guarantee
regardless of how careful the test itself is. It surfaced concretely when two
tests added in the same commit drove the same decimals-conflict code path and
incremented `STATS_EDGE_DECIMALS_CONFLICT_TOTAL` inside each other's
before/after windows — 2 failures in 6 runs. Serializing the two tests would
only hold until a third test touching the same counter was added.

- Cover the **behavior** the metric exists to confirm with database-state
  assertions instead (the affected row's `stats_processed`, `stats_asset_id`,
  the resulting aggregate row, etc.) — these are what actually pin the
  contract, and are already present alongside the metric assertion in most
  cases.
- Cover **metric emission itself** with a unit test on the pure decision
  function that increments the counter (no database, no shared process
  state), not through an integration test that shares a process-wide
  `lazy_static`.
- This applies to any `lazy_static`/`OnceLock`-backed `prometheus` metric,
  not just stats counters — the same construction is unsafe for any global
  counter/gauge read by more than one test in a binary.
