# Error Handling Rules

## Error Types

### Internal / Server-Boundary Code

Use `anyhow::Result` for server startup, config loading, and other flows
where callers do not need to branch on specific error variants (the primary
goal is propagation + context). This is the dominant pattern in
`stats-server/src/server.rs`, `settings.rs` (`Settings::build`),
`blockscout_waiter.rs`, and `runtime_setup.rs`:

```rust
async fn init_stats_db(settings: &Settings) -> anyhow::Result<Arc<DatabaseConnection>> {
    let db = Arc::new(
        blockscout_service_launcher::database::initialize_postgres::<stats::migration::Migrator>(
            &database_settings,
        )
        .await
        .context("stats DB")?,
    );
    Ok(db)
}
```

### Public / Cross-Boundary APIs

Use `thiserror` for structured error types where callers must distinguish
cases. Examples already in the codebase:

- `ChartError` (`stats/src/charts/chart.rs`) — the library-wide chart error
  type (`IndexerDB`, `StatsDB`, `ChartNotFound`, `NoCounterData`,
  `IntervalTooLarge`, `Internal`), converted `From<ReadError>` at the
  read-path boundary
- `ReadError` (`stats/src/charts/db_interaction/read/mod.rs`)
- `InitializationError` (`stats/src/update_group.rs`)
- `OnDemandReupdateError` (`stats-server/src/update_service.rs`)

```rust
#[derive(Error, Debug)]
pub enum ChartError {
    #[error("indexer database error: {0}")]
    IndexerDB(DbErr),
    #[error("chart {0} not found")]
    ChartNotFound(ChartKey),
    #[error("internal error: {0}")]
    Internal(String),
}
```

If you introduce a new custom error type (enum/struct) that crosses a module
or API boundary, derive it with `thiserror` instead of hand-writing
`Display`/`Error` impls. Convert between error layers with an explicit `From`
impl (see `impl From<ReadError> for ChartError`) rather than stringly-typed
conversions.

## Context

Add context when propagating an `anyhow`-flavored error, especially at
connection/IO boundaries:

```rust
// Good
let conn = Database::connect(opt).await.context("indexer DB")?;

// Avoid — no context, harder to diagnose later
.map_err(|e| e)?
```

## Logging Errors

Log at the point where an error is actually handled or turned into a
response, not silently during propagation. gRPC/HTTP handlers in
`read_service.rs`/`update_service.rs` convert a `ChartError` into a `Status`
at the boundary; log there if the failure is unexpected, not on every hop up
the call stack.

## API Error Sanitization

- Do not leak raw indexer/stats DB errors into gRPC/HTTP responses.
- `ChartError` variants are already coarse (`IndexerDB(DbErr)`,
  `StatsDB(DbErr)`) — map them to a generic `Status` at the service layer
  rather than serializing the inner `DbErr` to clients.
- Include stable identifiers (chart key, mode) in server-side logs for
  diagnosis.

## Panic Avoidance in Runtime Paths

- Avoid `unwrap()`/`expect()` in request handling and the update loop.
  `.unwrap()` is acceptable in `Settings::default()`/const-parsing contexts
  where the value is a hardcoded literal known to parse (see
  `SocketAddr::from_str("0.0.0.0:8050").unwrap()` in `settings.rs`), and in
  tests.
- One deliberate exception: `DataSource::all_dependencies_mutex_ids`
  (`stats/src/data_source/source.rs`) uses `assert!` to fail fast on a
  duplicate mutex id — a real invariant violation that should never occur
  given the type system, so failing loudly at startup is intentional there,
  not a runtime-path panic risk.
- Prefer `?` + context or explicit branching for recoverable failures in
  request/update paths.

## Recovery Patterns

- Use `.inspect_err()` for logging/metrics side effects at a handling
  boundary only (e.g. `auth.rs`'s ASCII-header-parsing warn), not chained
  repeatedly during propagation.
- Prefer explicit `match`/`if let` over silent `unwrap_or_default()` when the
  fallback path is itself meaningful (e.g. mode-specific defaults in
  `settings.rs`).
