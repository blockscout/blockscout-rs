use multichain_aggregator_entity::block_ranges;
use num_traits::ToPrimitive;
use rust_decimal::Decimal;
use sea_orm::{
    DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QuerySelect, sea_query::Expr,
};

#[derive(FromQueryResult, Debug)]
struct MinBlock {
    min_block: Option<Decimal>,
}

pub async fn get_min_block_multichain(multichain: &DatabaseConnection) -> Result<i64, DbErr> {
    let value = block_ranges::Entity::find()
        .select_only()
        .column_as(
            Expr::col(block_ranges::Column::MinBlockNumber).sum(),
            "min_block",
        )
        .into_model::<MinBlock>()
        .one(multichain)
        .await?;

    match value.and_then(|r| r.min_block).map(|m| m.to_i64().ok_or(m)) {
        Some(Ok(min_block)) => Ok(min_block),
        Some(Err(sum)) => {
            tracing::warn!(sum =? sum, "failed to convert min block sum to i64");
            Ok(i64::MAX)
        }
        None => {
            tracing::warn!("no block ranges found in multichain database");
            // set max so that if ranges appear, the reupdate is triggered
            Ok(i64::MAX)
        }
    }
}
