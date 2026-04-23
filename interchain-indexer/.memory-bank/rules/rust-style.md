# Rust Style Rules

Project-specific Rust style rules. For general conventions, see `../../RUST_CODE_STYLE_GUIDE.md`.

## Formatting

- Run `just format` before committing (applies `cargo sort` + `cargo fmt`)
- Import granularity is `Crate` level—group imports by crate

## Checking & Linting

- Run `just check` before committing (applies `cargo check` + `cargo clippy`)

## Config Structs

Always use `#[serde(deny_unknown_fields)]` for config structs to catch typos:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
    pub pull_interval_ms: u64,
}
```

## Proto Build Serde Attributes

In `interchain-indexer-proto/build.rs`, treat serde field attributes as behavior, not decoration.

- Do not add `#[serde(skip_serializing_if = "Option::is_none")]` to proto request fields just because they are optional.
- For HTTP/query input handling, omitted proto3 `optional` fields already deserialize as `None`; `skip_serializing_if` does not make an input optional and does not affect request validation.
- Use `skip_serializing_if` when serialized output shape matters, for example response messages or other structs intentionally serialized back to clients where omitting `null` fields is part of the API contract.
- When a request field truly needs deserialization behavior on omission, prefer the attribute that matches that behavior. Example: non-optional enum query fields may need `#[serde(default)]`; `skip_serializing_if` is not a substitute.
- Before adding any new serde field attribute in `build.rs`, check whether the message is used as input, output, or both, and document the reason in the surrounding task artifacts or code comment when it is not obvious.

## Logging

Use `tracing` with field-style syntax (static messages, dynamic fields):

```rust
// Good: static message, dynamic fields
tracing::info!(bridge_id, chain_id, count = logs.len(), "fetched logs");
tracing::error!(err = ?e, chain_id = ?chain_id, tx_hash = ?tx_hash, "processing failed");

// Bad: dynamic message string
tracing::info!("fetched {} logs for bridge {}", count, bridge_id);
```

Include relevant metadata in log messages (while avoiding any security-sensitive data) to help identify the source of the log and make it reproducible. For example, for errors related to processing a specific transaction, include the transaction hash as well as the chain ID.

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
