// SPDX-License-Identifier: LicenseRef-Blockscout

use std::future::Future;

use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DbErr, EntityTrait, Iterable, sea_query::OnConflict,
};

/// Max bind parameters PostgreSQL allows per prepared statement.
///
/// SeaORM's `insert_many` builds an `INSERT ... VALUES` statement with one bind
/// per column per row. `ON CONFLICT ... DO UPDATE` reuses `EXCLUDED` values and
/// does **not** introduce extra binds, so the total param count is exactly
/// `rows * column_count`. We therefore don't need an arbitrary safety margin;
/// we just ensure the computed batch size never exceeds the limit.
pub const PG_BIND_PARAM_LIMIT: usize = u16::MAX as usize;

/// Maximum keys per row-valued `IN` statement — `(a, b) IN ((…),(…),…)`.
///
/// Deliberately **not** derived from [`PG_BIND_PARAM_LIMIT`]. PostgreSQL expands
/// a row-valued `IN` into an `OR`-tree of row comparisons and recurses over it
/// while parsing and planning, so the binding ceiling is `max_stack_depth`, not
/// the bind-parameter count, and no bind arithmetic can express it. Sizing by
/// `PG_BIND_PARAM_LIMIT / width` is bind-safe but *not* stack-safe: it leaves a
/// row-valued `IN` at up to 32 767 tuples, roughly four times over the measured
/// ceiling.
/// See `.memory-bank/research/stats-projection-unbatched-pks-lookup-crash.md`.
///
/// **Measured** against this repo's PostgreSQL at the default
/// `max_stack_depth = 2048kB`: a parameterised row-valued `IN` overflows the
/// planner stack between 7 500 and 8 000 tuples. The threshold is set by the
/// *length* of the `OR`-list, not the width of each tuple — a 5-column key
/// overflows at the same count as a 2-column one — so one constant covers every
/// shape. 2 000 keeps a ~3.8x margin. At that size the widest tuple here
/// (5 columns) uses 10 000 of the 65 535 bind parameters (~15%), so bind count
/// is never the binding constraint.
///
/// Do not raise this without re-measuring: the margin is against a threshold
/// that moves with `max_stack_depth` and with the deployment's actual thread
/// stack limit.
///
/// Use this for row-valued `IN` only. For flat statements (`INSERT … VALUES`,
/// single-column `is_in()`), bind arithmetic is correct — use
/// [`run_in_batches`] / [`batched_upsert`].
pub const ROW_IN_KEY_CHUNK: usize = 2_000;

/// Widest row-valued `IN` tuple in this codebase: `EdgeKey` in
/// `stats/projection.rs`, five columns.
const WIDEST_ROW_IN_TUPLE: usize = 5;

const _: () = assert!(
    ROW_IN_KEY_CHUNK * WIDEST_ROW_IN_TUPLE < PG_BIND_PARAM_LIMIT,
    "row-`IN` chunk must leave bind headroom for the widest key tuple plus filter constants"
);

/// Calculate a batch size that keeps total bind parameters under PostgreSQL's
/// limit for a given row width.
fn batch_size_for_width(width: usize) -> Result<usize, DbErr> {
    if width == 0 {
        Err(DbErr::Custom(
            "cannot compute batch size for zero column width".to_string(),
        ))?;
    }

    let batch_size = PG_BIND_PARAM_LIMIT / width;
    if batch_size == 0 {
        Err(DbErr::Custom(format!(
            "row width {width} exceeds PostgreSQL bind parameter limit \
             {PG_BIND_PARAM_LIMIT}"
        )))?;
    }

    Ok(batch_size)
}

/// Generic batched executor that keeps total bind parameters under PostgreSQL's limit.
pub async fn run_in_batches<'a, T, F, Fut>(
    items: &'a [T],
    row_width: usize,
    mut f: F,
) -> Result<(), DbErr>
where
    F: FnMut(&'a [T]) -> Fut,
    Fut: Future<Output = Result<(), DbErr>>,
{
    let batch_size = batch_size_for_width(row_width)?;
    for batch in items.chunks(batch_size) {
        f(batch).await?;
    }
    Ok(())
}

/// Batched executor that chunks at an explicit `chunk_size` instead of deriving
/// one from a row width. Use for row-valued `IN` statements, where the ceiling
/// is planner stack depth rather than bind count — see [`ROW_IN_KEY_CHUNK`].
///
/// A `chunk_size` of 0 is clamped to 1 rather than returning an error: this runs
/// inside the shared maintenance transaction, where a new `Err` path would roll
/// back cursor progress (`.memory-bank/rules/error-handling.md`).
pub async fn run_in_chunks<'a, T, F, Fut>(
    items: &'a [T],
    chunk_size: usize,
    mut f: F,
) -> Result<(), DbErr>
where
    F: FnMut(&'a [T]) -> Fut,
    Fut: Future<Output = Result<(), DbErr>>,
{
    for batch in items.chunks(chunk_size.max(1)) {
        f(batch).await?;
    }
    Ok(())
}

/// Upserts models in batches respecting PostgreSQL's bind parameter limit.
pub async fn batched_upsert<A, E>(
    db: &impl ConnectionTrait,
    models: &[A],
    on_conflict: OnConflict,
) -> Result<(), DbErr>
where
    A: ActiveModelTrait<Entity = E> + Clone + Send,
    E: EntityTrait,
{
    let column_count = E::Column::iter().count();
    run_in_batches(models, column_count, |batch| async {
        E::insert_many(batch.to_vec())
            .on_empty_do_nothing()
            .on_conflict(on_conflict.clone())
            .exec(db)
            .await
            .map(|_| ())
    })
    .await
}
