use crate::kv_storage;
use blockscout_db::entity::blocks;
use migration::DbErr;
use sea_orm::{prelude::*, sea_query::Expr, DatabaseConnection, FromQueryResult, QuerySelect};

const MIN_BLOCK_KEY: &str = "min_block";

pub async fn is_blockscout_indexing(
    blockscout: &DatabaseConnection,
    db: &DatabaseConnection,
) -> Result<(bool, i64), DbErr> {
    let min_block_blockscout = get_min_block_blockscout(blockscout).await?;
    let min_block_saved = get_min_block_saved(db).await?;
    tracing::info!(
        min_block_blockscout = min_block_blockscout,
        min_block_saved = min_block_saved,
        "checking min block in blockscout database"
    );
    let is_indexing = min_block_blockscout != min_block_saved;
    Ok((is_indexing, min_block_blockscout))
}

#[derive(FromQueryResult)]
struct MinBlock {
    min_block: i64,
}

async fn get_min_block_blockscout(blockscout: &DatabaseConnection) -> Result<i64, DbErr> {
    let min_block = blocks::Entity::find()
        .select_only()
        .column_as(Expr::col(blocks::Column::Number).min(), "min_block")
        .filter(blocks::Column::Consensus.eq(true))
        .into_model::<MinBlock>()
        .one(blockscout)
        .await?;

    min_block
        .map(|r| r.min_block)
        .ok_or_else(|| DbErr::RecordNotFound("no blocks found in blockscout database".into()))
}

async fn get_min_block_saved(db: &DatabaseConnection) -> Result<i64, DbErr> {
    let value = kv_storage::get_value(db, MIN_BLOCK_KEY).await?;
    let value = match value {
        Some(v) => v.parse::<i64>().map_err(|e| {
            DbErr::Type(format!(
                "cannot parse value in kv_storage with key '{MIN_BLOCK_KEY}': {e}",
            ))
        })?,
        None => i64::MAX,
    };
    Ok(value)
}

pub async fn set_min_block_saved(db: &DatabaseConnection, value: i64) -> Result<(), DbErr> {
    kv_storage::set_value(db, MIN_BLOCK_KEY, &value.to_string()).await
}

#[cfg(test)]
mod tests {
    // TODO
}
