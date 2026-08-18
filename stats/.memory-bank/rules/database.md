---
paths:
  - "stats/src/charts/db_interaction/**"
  - "stats/src/utils.rs"
  - "stats/entity/**"
  - "stats/migration/**"
globs:
  - "stats/src/charts/db_interaction/**"
  - "stats/src/utils.rs"
  - "stats/entity/**"
  - "stats/migration/**"
---

# Database Rules

Rules for the stats DB (`entity`/`migration` crates, tables `charts` and
`chart_data`) and for raw-SQL statements against indexer databases.

There are two very different "database" concerns in this codebase — keep
them distinct:

1. The **stats DB** (this service's own Postgres) — written via SeaORM
   `ActiveModel`s in `stats/src/charts/db_interaction/write.rs`.
2. **Indexer DB(s)** (Blockscout / multichain-aggregator / interchain-indexer
   / zetachain-cctx) — read-only, via hand-built raw SQL `Statement`s in
   `stats/src/charts/db_interaction/read/` and individual chart files.

## Upsert Pattern (Stats DB)

Use `on_conflict()` for idempotent writes, exactly as
`stats/src/charts/db_interaction/write.rs` does:

```rust
// chart_data upsert: recompute-safe, keyed by (chart_id, date)
chart_data::Entity::insert_many(data)
    .on_conflict(
        sea_query::OnConflict::columns([
            chart_data::Column::ChartId,
            chart_data::Column::Date,
        ])
        .update_column(chart_data::Column::Value)
        .update_column(chart_data::Column::MinBlockscoutBlock)
        .to_owned(),
    )
    .exec(db)
    .await?;

// charts row creation: idempotent, do nothing if already present
charts::Entity::insert(charts::ActiveModel { .. })
    .on_conflict(
        sea_query::OnConflict::columns([charts::Column::Name, charts::Column::Resolution])
            .do_nothing()
            .to_owned(),
    )
    .exec(db)
    .await?;
```

## Building Raw SQL Against Indexer DBs

Indexer-DB queries are built as raw `sea_orm::Statement`s, not SeaORM query
builders, because the indexer schemas (Blockscout, multichain-aggregator,
interchain-indexer, zetachain-cctx) are external and vary by `Mode`. Use the
macros in `stats/src/utils.rs` rather than hand-formatting SQL strings:

```rust
sql_with_range_filter_opt!(
    DbBackend::Postgres,
    r#"
        SELECT date(blocks.timestamp) as date, COUNT(*)::TEXT as value
        FROM public.blocks
        WHERE blocks.timestamp != to_timestamp(0) AND consensus = true {filter}
        GROUP BY date;
    "#,
    [],
    "blocks.timestamp",
    range
)
```

This expands to `sea_orm::Statement::from_sql_and_values(...)`, with the
range/multichain filter's placeholder numbering computed from
`values.len() + 1` so it always lines up with whatever fixed values came
before it (`produce_filter_and_values`/`produce_day_filter_and_values` in
`stats/src/utils.rs`). There's a sibling `sql_with_multichain_filter_opt!`
for the `multichain_filter` chain-id predicate. When a query needs both a
range filter and something else dynamic, build the extra predicate the same
way — compute its placeholder start from the already-pushed value count, not
a hardcoded number.

For queries that don't fit the macro shape (e.g. `total_blocks`'s counter
query), it's normal to use SeaORM's typed query builder directly against an
indexer entity re-exported from a sibling crate
(`blockscout_db::entity::blocks`) when one exists, falling back to
`FromQueryResult` + a raw `Statement` otherwise.

## Mode-Dispatching Read Helpers

Several read helpers branch on `Mode` to hit the right indexer schema, e.g.
`get_min_date` (`stats/src/charts/db_interaction/read/mod.rs`):

```rust
pub async fn get_min_date(indexer_db: &DatabaseConnection, mode: Mode) -> Result<NaiveDateTime, DbErr> {
    match mode {
        Mode::Interchain => get_min_date_interchain(indexer_db).await,
        Mode::MultichainAggregator => get_min_date_multichain(indexer_db).await,
        _ => get_min_date_blockscout(indexer_db).await, // Blockscout, Zetachain
    }
}
```

When adding a mode-aware read helper, follow this shape — a `match` on `Mode`
dispatching to a per-schema submodule (`read::blockscout`, `read::interchain`,
`read::multichain`) — rather than branching on config flags scattered through
call sites.

## Choosing the Indexer Connection (`db_choice`)

`stats/src/data_source/kinds/remote_db/db_choice.rs` provides
`UsePrimaryDB`/`UseZetachainCctxDB` markers (via `impl_db_choice!`) that a
`RemoteDatabaseSource`'s statement type declares, selecting whether its query
runs against `cx.indexer_db` or `cx.second_indexer_db`. `UseZetachainCctxDB`
only makes sense for `Zetachain`-mode-only charts (the second indexer DB is
only ever connected in that mode — see `.memory-bank/gotchas.md`).

## Entity Generation

- SeaORM entities for the stats DB live in `stats/stats/entity`
  (`entity` crate); migrations in `stats/stats/migration` (`migration` crate).
- Regenerate with the `generate-entities` recipe in the `stats/stats` crate's
  own justfile (invoked via `just restart-generate-entities` at the workspace
  root, which also brings up a fresh Postgres and runs migrations first).

## Timestamps

- `chart_data`/`charts` timestamps use `chrono::DateTime<Tz>` normalized to a
  fixed UTC offset before writing (`at.with_timezone(&chrono::Utc.fix())` in
  `write.rs::set_last_updated_at`).
- `last_updated_at` is application-managed (explicit `Set(...)`), not a
  DB-side trigger/default.

## Client-Facing DB Errors

- Never propagate a raw `DbErr` string into a gRPC/HTTP response — map
  through `ChartError::{IndexerDB, StatsDB}` and let the service layer turn
  that into a generic `Status`, per `.memory-bank/rules/error-handling.md`.
