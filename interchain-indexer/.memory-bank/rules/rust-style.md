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
