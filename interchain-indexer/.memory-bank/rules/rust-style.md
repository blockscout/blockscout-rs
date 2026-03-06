# Rust Style Rules

Project-specific Rust style rules. For general conventions, see `../../RUST_CODE_STYLE_GUIDE.md`.

## Formatting

- Run `just format` before committing (applies `cargo sort` + `cargo fmt`)
- Import granularity is `Crate` level—group imports by crate

## Config Structs

Always use `#[serde(deny_unknown_fields)]` for config structs to catch typos:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
    pub pull_interval_ms: u64,
}
```

## Logging

Use `tracing` with field-style syntax (static messages, dynamic fields):

```rust
// Good: static message, dynamic fields
tracing::info!(bridge_id, chain_id, count = logs.len(), "fetched logs");
tracing::error!(err = ?e, "processing failed");

// Bad: dynamic message string
tracing::info!("fetched {} logs for bridge {}", count, bridge_id);
```

## Project-Specific Naming

| Element | Convention | Example |
|---------|------------|---------|
| Traits | PascalCase, descriptive | `CrosschainIndexer`, `Consolidate` |
| Type aliases | PascalCase | `TokenKey = (i64, Vec<u8>)` |
| Constants | SCREAMING_SNAKE | `PG_BIND_PARAM_LIMIT` |

## Domain Types over Storage Types

Use domain-specific types in logic; cast to storage types only at the DB write boundary.

- EVM chain IDs: `ChainId` (`u64` from alloy) everywhere except persistence, where `i64::try_from(chain_id)?` is explicit and isolated. `i64` is a PostgreSQL artefact — keep it there.

## Early Returns: prefer `Err(...)?` over `return Err(...)`

`return Err(...)` is imperative; prefer the functional equivalents:

- `Err(anyhow!("..."))?` — inline one-off errors
- `bail!("...")` — early exit with an anyhow error (most concise)
- `ensure!(condition, "...")` — guard conditions (`if !cond { bail! }` in one line)

```rust
// Bad
if x.is_empty() { return Err(anyhow!("empty")); }

// Good
ensure!(!x.is_empty(), "empty");
```

## Functional Style for Boolean Logic

Prefer `match` + guard arms and `Option::map_or` over imperative `if`/`return` chains:

```rust
match (src_configured, dst_configured) {
    (true, true)   => true,
    (false, false) => false,
    _ if !process_unknown => false,
    (true, false)  => primary.map_or(true, |p| src == p),
    (false, true)  => primary.map_or(true, |p| dst == p),
}
```

## Descriptive Parameter Names

Name parameters after semantic role, not position. Prefer `source_chain_id`/`dest_chain_id` over `chain_a`/`chain_b`.
