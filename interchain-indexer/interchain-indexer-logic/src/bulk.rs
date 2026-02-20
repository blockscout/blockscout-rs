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
