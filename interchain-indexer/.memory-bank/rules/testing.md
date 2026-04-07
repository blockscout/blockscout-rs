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
# Runs all tests with a temporary Postgres instance
just test-with-db

# Specific test
just test test_name
```

Do not use `just test` since most likely it will failed without connected database.


## Test Naming

- Describe what is being tested and expected outcome
- Format: `test_<function>_<scenario>_<expected>`
- Example: `test_consolidate_incomplete_message_returns_none`

## Assertions

- Prefer specific assertions over generic `assert!`
- Use `assert_eq!` with descriptive messages
- For complex assertions, use `pretty_assertions` crate
