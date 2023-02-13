use blockscout_db::entity::blocks;
use sea_orm::{prelude::*, sea_query, FromQueryResult, QuerySelect};

mod dependent;
mod full;
mod partial;
pub use dependent::{parse_and_growth, parse_and_sum, ChartDependentUpdater};
pub use full::ChartFullUpdater;
pub use partial::ChartPartialUpdater;

#[derive(FromQueryResult)]
struct MinBlock {
    min_block: i64,
}

pub async fn get_min_block_blockscout(blockscout: &DatabaseConnection) -> Result<i64, DbErr> {
    let min_block = blocks::Entity::find()
        .select_only()
        .column_as(
            sea_query::Expr::col(blocks::Column::Number).min(),
            "min_block",
        )
        .filter(blocks::Column::Consensus.eq(true))
        .into_model::<MinBlock>()
        .one(blockscout)
        .await?;

    min_block
        .map(|r| r.min_block)
        .ok_or_else(|| DbErr::RecordNotFound("no blocks found in blockscout database".into()))
}
