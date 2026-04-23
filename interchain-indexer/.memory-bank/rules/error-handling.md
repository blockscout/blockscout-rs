# Error Handling Rules

## Error Types

### Internal Code

Use `anyhow::Result` for internal flows where callers do not need to branch on
specific error variants (the primary goal is propagation + logging context).
If callers must distinguish scenarios (for example, `NotFound` vs
`RateLimited`), prefer a typed error enum and convert at the boundary.

```rust
pub async fn process(&self) -> anyhow::Result<()> {
    let data = self.fetch()
        .await
        .with_context(|| "failed to fetch data")?;
    Ok(())
}
```

### Public APIs

Use `thiserror` for structured error types at API boundaries:

If you introduce a new custom error type (enum/struct), derive it with the
`thiserror` crate instead of hand-writing `Display`/`Error` impls.

```rust
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}
```

## Context

Always add context when propagating errors:

```rust
// Good
.with_context(|| format!("failed to process chain {}", chain_id))?

// Bad
.map_err(|e| e)?  // No context added
```

## Logging Errors

Log at the handling point, not during propagation:

```rust
// Good: log where you handle it
match self.process().await {
    Ok(_) => {},
    Err(e) => {
        tracing::error!(err = ?e, "processing failed");
        // Handle or return
    }
}

// Bad: log during propagation
let result = self.fetch().await.inspect_err(|e| {
    tracing::error!(err = ?e, "fetch failed");  // May log twice
})?;
```

## API Error Sanitization

- Do not expose internal DB/provider errors in API responses.
- Return generic internal-failure messages to clients.
- Log full diagnostic context server-side (`tracing::error!(err = ?e, ...)`).
- Include stable identifiers (chain ID, bridge ID, message ID) in logs where applicable.

## Panic Avoidance in Runtime Paths

- Avoid `unwrap()`/`expect()` in server startup, request handling, and indexer loops.
- Prefer `?` + context or explicit branching for recoverable failures.
- Validate external input before parsing/conversion.

## Recovery Patterns

- Use `inspect_err()` only at handling boundaries (e.g., metrics/logging), not during propagation.
- Use `unwrap_or_default()` only for truly optional values
- Prefer explicit handling over silent defaults

Numeric/time correctness:
- Prefer checked/saturating arithmetic for block and pagination calculations.
- Use Euclidean division (`div_euclid`/`rem_euclid`) for negative timestamp conversions.
