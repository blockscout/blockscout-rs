# Rust Style Rules

Project-specific Rust style rules for the `stats` workspace (`stats`,
`stats-server`, `stats-proto`, `entity`, `migration`, `env-docs-generation`).

## Formatting

- Run `just format` before committing — applies `cargo sort --workspace` (if
  `cargo-sort` is installed) then `cargo fmt --all -- --config imports_granularity=Crate`
- Import granularity is `Crate` level — group imports by crate

## Checking & Linting

- Run `just check` before committing —
  `cargo check` + `cargo clippy --all --all-targets --all-features -- -D warnings`
- `just format-check` runs both (`format` then `check`) in one step

## License Header

Every Rust source file in this workspace starts with:

```rust
// SPDX-License-Identifier: LicenseRef-Blockscout
```

Keep this as the first line of any new file.

## Config Structs

Settings/config structs consistently use `#[serde(default, deny_unknown_fields)]`
to catch typos and let every field fall back to `Default`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LimitsSettings {
    pub requested_points_limit: u32,
}
```

See `stats-server/src/settings.rs` (`Settings`, `LimitsSettings`,
`StartConditionSettings`, `ToggleableThreshold`, `ToggleableCheck`) for the
pattern, including nested toggleable sub-settings with their own `Default` impls.

## Logging

Logging in this codebase mixes structured (field-style) and plain
interpolated messages — `tracing::info!(response_content =? response.content, "User ops are disabled")`
alongside `tracing::info!("Observed new indexing status: {:?}", status)`
both appear in `stats-server/src/blockscout_waiter.rs`. When adding a new log
call with reusable structured context (chart key, chain id, mode, error),
prefer the field-style form so the fields are queryable independently of the
message text:

```rust
// Preferred for structured context
tracing::warn!(update_group = self.name(), mutex_id = name, "did not lock mutex");

// Also fine for a one-off human-readable message
tracing::info!("Observed new indexing status: {:?}", status);
```

## Naming

| Element | Convention | Example |
|---------|------------|---------|
| Traits | PascalCase, descriptive | `DataSource`, `ChartProperties`, `RemoteQueryBehaviour` |
| Chart/data-source type aliases | PascalCase, composed via generics | `NewBlocksInt = MapParseTo<StripExt<NewBlocks>, i64>` |
| Constants | SCREAMING_SNAKE | `LINKED_STATS_MAX_HOPS_HARD_CAP`, `NEW_TXNS_WINDOW_RANGE` |

## Chart Declaration Pattern

Chart implementation files (`stats/src/charts/{counters,lines}/**`) share a
consistent shape — see `stats/src/charts/lines/blockscout_instance/blocks/new_blocks.rs`
and `stats/src/charts/counters/blockscout_instance/total_blocks.rs`:

1. `use crate::chart_prelude::*;` (the one import most files need — see
   `stats/src/preludes.rs`)
2. A statement/behaviour type (`XStatement` implementing `StatementFromRange`
   or `XQueryBehaviour` implementing `RemoteQueryBehaviour`), built with
   `sql_with_range_filter_opt!`/raw SeaORM query builders
3. A `RemoteDatabaseSource<...>` type alias wrapping the behaviour
4. `struct Properties;` implementing `Named` + `ChartProperties`
   (`chart_type()`, optionally `missing_date_policy()`/
   `indexing_status_requirement()`)
5. `define_and_impl_resolution_properties!` for weekly/monthly/yearly
   variants when the chart has them
6. The public chart type alias (`DirectVecLocalDbChartSource<...>` /
   `DirectPointLocalDbChartSource[WithEstimate]<...>`), plus any
   `MapParseTo`/`StripExt`/`SumLowerResolution` intermediate aliases needed by
   dependent charts or lower resolutions
7. `#[cfg(test)] mod tests { ... }` using `stats::tests::*` helpers

Follow this shape for new charts rather than inventing a new structure.

## Early Returns

Both `?`-propagation and explicit `return Err(...)` appear in this codebase;
neither is enforced over the other. `anyhow::Context`/`.context(...)` is the
established pattern for adding context to a propagated error at the
`stats-server` boundary (`server.rs`, `blockscout_waiter.rs`):

```rust
let db = Database::connect(opt).await.context("indexer DB")?;
```

`ensure!`/`bail!` are used sparingly for early guard conditions
(`stats-server/src/blockscout_waiter.rs`, `runtime_setup.rs`); prefer them
over a manual `if !cond { return Err(...) }` when the check is a one-liner.

## Descriptive Parameter Names

Name parameters after semantic role, not position — e.g.
`interchain_primary_id`, `multichain_filter`, `indexer_db` /
`second_indexer_db` rather than generic `db_a`/`db_b`.
