use multichain_aggregator_entity::block_ranges;
use sea_orm::{
    DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QuerySelect, sea_query::Expr,
};

#[derive(FromQueryResult)]
struct MinBlock {
    min_block: Option<i32>,
}

pub async fn get_min_block_multichain(multichain: &DatabaseConnection) -> Result<i64, DbErr> {
    let not_found_value = i32::MAX;
    let min_block = block_ranges::Entity::find()
        .select_only()
        .column_as(
            Expr::col(block_ranges::Column::MaxBlockNumber).min(),
            "min_block",
        )
        .into_model::<MinBlock>()
        .one(multichain)
        .await
        .inspect_err(|e| {
            tracing::warn!("could not find min block from multichain db: {:?}", e);
        })
        .unwrap_or(Some(MinBlock {
            min_block: Some(not_found_value),
        }));

    match min_block.map(|r| r.min_block).flatten() {
        Some(min_block) => Ok(min_block as i64),
        None => {
            tracing::warn!("no block ranges found in multichain database");
            // set max so that if ranges appear, the reupdate is triggered
            Ok(not_found_value as i64)
        }
    }
}
