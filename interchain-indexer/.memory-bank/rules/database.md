---
paths:
  - "interchain-indexer-logic/src/database.rs"
  - "interchain-indexer-logic/src/bulk.rs"
  - "interchain-indexer-logic/src/pagination.rs"
  - "interchain-indexer-entity/**"
  - "interchain-indexer-migration/**"
globs:
  - "interchain-indexer-logic/src/database.rs"
  - "interchain-indexer-logic/src/bulk.rs"
  - "interchain-indexer-logic/src/pagination.rs"
  - "interchain-indexer-entity/**"
  - "interchain-indexer-migration/**"
---

# Database Rules

Rules for SeaORM entities, migrations, and database operations.

## Upsert Pattern

Always use `on_conflict()` for idempotent inserts:

```rust
Entity::insert_many(models)
    .on_conflict(
        OnConflict::column(Column::Id)
            .update_columns([Column::Field1, Column::Field2])
            .value(Column::UpdatedAt, Expr::current_timestamp())
            .to_owned()
    )
    .exec(db)
    .await?
```

## Batching

Respect PostgreSQL's bind parameter limit (65535):

```rust
const PG_BIND_PARAM_LIMIT: usize = 65535;

let batch_size = PG_BIND_PARAM_LIMIT / columns_per_row;
for batch in items.chunks(batch_size) {
    upsert_batch(batch).await?;
}
```

Use `batched_upsert()` or `run_in_batches()` from `bulk.rs`.

## Entity Generation

- Auto-generated entities go in `interchain-indexer-entity/src/codegen/`
- Manual customizations go in `interchain-indexer-entity/src/manual/`
- Regenerate with `just generate-entities` (overwrites codegen/)
- Ensure local DB is running and schema is current before generation:
  `just start-postgres` + `just migrate-up`
- If generation fails due stale local DB state, restart and re-run migrations
  before generation

## Migrations

- Create new migrations with `just new-migration <name>`
- Use `from_sql()` helper for raw SQL when needed
- Test migrations with `just migrate-fresh`

## Type Conversions

Implement `From` for ActiveModel conversions:

```rust
impl From<Config> for entity::ActiveModel {
    fn from(config: Config) -> Self {
        Self {
            field: Set(config.field),
            ..Default::default()
        }
    }
}
```

## Timestamps

- Use `Expr::current_timestamp()` for `updated_at` in upserts
- Store timestamps as `DateTime<Utc>` (chrono)
- Database stores as `TIMESTAMP WITH TIME ZONE`
- Prefer application-managed `updated_at` writes for deterministic behavior

## Pagination

- Use `ListMarker` trait for cursor-based pagination
- Token format: `BASE64(direction | timestamp | id | bridge_id)`
- Never expose internal IDs; use opaque tokens
- Cursor marker fields must exactly match the SQL `ORDER BY` + tie-breaker fields
- Marker decode/encode order must be stable and deterministic across pages

## Client-Facing DB Errors

- Never propagate raw DB error messages to API clients
- Map DB failures to sanitized internal-error responses
- Log full DB error diagnostics with `tracing` at the service boundary

## Hand-Built Positional SQL Placeholders Must Leave The Counter Advanced

When building raw SQL with manual `$1, $2, ...` placeholders (as the stats
endpoints in `database.rs` do for filter predicates), a helper that appends a
predicate must advance the shared placeholder counter by exactly the number
of values it pushed — not just push the values themselves:

```rust
// Good: counter reflects every value actually pushed, so a predicate
// appended afterward starts numbering from the correct next slot.
fn push_predicate(where_parts: &mut Vec<String>, values: &mut Vec<Value>, placeholder: &mut usize, ids: &[i64]) {
    let start = *placeholder;
    for id in ids {
        values.push((*id).into());
        *placeholder += 1;
    }
    where_parts.push(format!("col IN ({})", (start..*placeholder).map(|n| format!("${n}")).collect::<Vec<_>>().join(", ")));
}
```

A block that pushes placeholders without advancing the counter (or advances
it by the wrong amount) is harmless in isolation — the bug only manifests
once a later call appends a further predicate after it, at which point the
new predicate's `$N` numbering silently misbinds against the wrong value.
This is easy to ship undetected because nothing before that point exercises
the interaction.

**Enforce with a test that appends a further dummy predicate after the
helper under test and asserts the resulting placeholder numbers are
contiguous** — this is the only way the invariant is actually exercised; a
test that only checks the helper's own output in isolation cannot catch it.
See `push_indexed_pairs_predicate` / `push_zero_chains_guard_predicate` in
`interchain-indexer-logic/src/database.rs` and their
`test_*_contiguous_after_prior_predicates` /
`test_*_contiguous_with_predicate_appended_after` tests for the pattern —
the guard-predicate test was written specifically because an earlier revision
of that helper left the counter stale in a way that was invisible only
because nothing was ever built after it.
