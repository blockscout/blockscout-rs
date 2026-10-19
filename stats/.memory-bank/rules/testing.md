# Testing Rules

Rules for writing and organizing tests in the `stats` workspace.

## Test Attributes

```rust
// Async tests
#[tokio::test]

// Database-backed tests: keep `#[tokio::test]` and `#[ignore = "needs database to run"]`
// so they only run when explicitly requested (typically via `just test-with-db`)
#[tokio::test]
#[ignore = "needs database to run"]
```

Almost every chart test in `stats/src/charts/**` is
`#[ignore = "needs database to run"]` — this is the norm here, not an
exception.

## The `test-utils` Feature

`stats`'s test helpers (`stats/src/tests/`) are gated by the `test-utils`
Cargo feature (`stats/Cargo.toml`), which pulls in `tracing-subscriber`,
`blockscout-service-launcher`, `pretty_assertions`, `wiremock`,
`hex-literal`, `serde_json` as test-only dependencies. `stats/src/lib.rs`
exposes the `tests` module under
`#[cfg(any(feature = "test-utils", test))]` — so it's available inside the
crate's own `#[cfg(test)]` modules without enabling the feature, but another
crate that wants to reuse these helpers must enable `test-utils` explicitly.

## Database Tests

Use `blockscout_service_launcher::test_database::TestDbGuard` for isolated
database instances — one per stats DB and one per indexer DB, both spun up
per test:

```rust
use blockscout_service_launcher::test_database::TestDbGuard;

#[tokio::test]
#[ignore = "needs database to run"]
async fn update_new_blocks_recurrent() {
    let (db, blockscout) = init_db_all("update_new_blocks_recurrent").await;
    // ... use db / blockscout
}
```

`test_name` (the string passed to `init_db_all`/`simple_test_chart`/etc.)
must be unique across the test suite to avoid DB name clashes — this is
called out directly in `stats/src/tests/simple_test.rs`'s doc comment.

## Chart Test Helpers (`stats/src/tests/`)

- `init_db.rs` — `init_db_all` (Blockscout), `init_db_all_interchain`,
  `init_db_all_multichain`, `init_db_zetachain_cctx` — set up the stats DB +
  the matching indexer DB(s) for a given mode.
- `mock_blockscout.rs`, `mock_blockscout_filecoin.rs`, `mock_interchain.rs`,
  `mock_multichain.rs`, `mock_zetachain_cctx.rs` — fill an indexer DB with a
  shared fixture dataset (`fill_mock_blockscout_data`, etc.).
- `simple_test.rs` — the highest-level helpers:
  - `simple_test_chart::<C>(test_name, expected)` — full init + update +
    query round trip against `Mode::Blockscout` with mock data, asserting
    the resulting points match `expected: Vec<(&str, &str)>`
    (`(date_str, value_str)` pairs)
  - `simple_test_chart_filecoin`, `simple_test_chart_multichain`,
    `simple_test_chart_with_migration_variants` (runs the same expectations
    against both `IndexerMigrations::empty()` and `::latest()`) — mode/feature
    variants of the same pattern
  - `get_counter::<C>(&cx)` — fetch a counter's current value
  - `test_counter_fallback::<C>(test_name)` — exercises a counter's
    `ValueEstimation` fallback path
- `point_construction.rs` — helpers for building expected
  `ExtendedTimespanValue`/`TimespanValue`/`DateValue` test data and parsing
  fixed timestamps (`dt("2022-11-12T11:00:00")`).

Prefer `simple_test_chart`/`get_counter`/`test_counter_fallback` for a new
chart's tests before writing bespoke `init_recursively`/`update_recursively`
call sequences by hand — most existing chart tests use them, and hand-rolled
tests (like the three explicit `update_new_blocks_*` tests in `new_blocks.rs`)
exist specifically to exercise a subtlety `simple_test_chart` can't (partial
vs. full re-update, stale `chart_data` rows being overwritten vs. kept).

## Server Integration Tests

`stats-server/tests/it/` holds full-server integration tests:

- `common.rs` — shared setup
- `mock_blockscout_simple/` — one module per API-shape scenario
  (`stats_full.rs`, `stats_no_specific.rs`, `stats_not_indexed.rs`,
  `stats_not_updated.rs`, `stats_filecoin_enabled.rs`, `common_tests.rs`)
- `mock_blockscout_reindex.rs`, `linked_stats.rs` — targeted scenarios

## Test Organization

- Unit tests: `#[cfg(test)] mod tests { ... }` at the end of the module
  (every chart implementation file follows this)
- Server integration tests: `stats-server/tests/it/`

## Running Tests

```bash
# Runs cargo test with --include-ignored — attempts DB-backed tests too
just test [test_name]

# Self-contained: starts a disposable Postgres, then runs `just test` against it
just test-with-db [test_name]
```

`just test` always passes `--include-ignored`, so a bare `just test` will
attempt every `#[ignore = "needs database to run"]` test and fail without a
reachable `DATABASE_URL`. Prefer `just test-with-db` for a full run; use
`just test <specific_non_db_test_name>` for a quick single non-DB test. See
`.memory-bank/gotchas.md` for the `DATABASE_URL`-override caveat in
`stats/justfile`.

## Test Naming

- Describe what is being tested and the scenario, e.g.
  `update_new_blocks_recurrent`, `update_new_blocks_fresh`,
  `update_new_blocks_last`, `total_blocks_fallback`.
- For chart tests built on `simple_test_chart`, the test name string passed
  in should match the `#[tokio::test] async fn` name (both must be unique
  across the suite, since the DB name is derived from it).

## Assertions

- Prefer `pretty_assertions::assert_eq!` (already a dependency behind
  `test-utils`) over the standard library's `assert_eq!` for readable diffs
  on the vector/point comparisons chart tests produce.
- Prefer specific assertions (`assert_eq!` with the actual expected value)
  over a bare `assert!(...)`.
