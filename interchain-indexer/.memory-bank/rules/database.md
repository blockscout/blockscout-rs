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
