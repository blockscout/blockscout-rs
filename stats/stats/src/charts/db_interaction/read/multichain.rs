use multichain_aggregator_entity::block_ranges;
use sea_orm::{
    DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QuerySelect, sea_query::Expr,
};

#[derive(FromQueryResult, Debug)]
struct MinBlock {
    min_block: i64,
}

pub async fn get_min_block_multichain(multichain: &DatabaseConnection) -> Result<i64, DbErr> {
    let not_found_value = i64::MAX;
    let min_blocks = block_ranges::Entity::find()
        .select_only()
        .column_as(
            Expr::col(block_ranges::Column::MinBlockNumber).min(),
            "min_block",
        )
        .group_by(block_ranges::Column::ChainId)
        .into_model::<MinBlock>()
        .all(multichain)
        .await?;
    let min_block = min_blocks
        .iter()
        .map(|m| m.min_block)
        .reduce(i64::saturating_add)
        .unwrap_or_else(|| {
            tracing::warn!("no block ranges found in multichain database");
            not_found_value
        });
    Ok(min_block)
}
