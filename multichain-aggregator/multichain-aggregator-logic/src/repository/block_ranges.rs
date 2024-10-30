use crate::types::block_ranges::BlockRange;
use entity::block_ranges::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    prelude::Expr, sea_query::OnConflict, ActiveValue::NotSet, ConnectionTrait, EntityTrait,
};

pub async fn upsert_many<C>(db: &C, block_ranges: Vec<BlockRange>) -> anyhow::Result<()>
where
    C: ConnectionTrait,
{
    if block_ranges.is_empty() {
        return Ok(());
    }

    let block_ranges = block_ranges.into_iter().map(|block_range| {
        let model: Model = block_range.into();
        let mut active: ActiveModel = model.into();
        active.created_at = NotSet;
        active.updated_at = NotSet;
        active
    });

    Entity::insert_many(block_ranges)
        .on_conflict(
            OnConflict::column(Column::ChainId)
                .update_columns([Column::MinBlockNumber, Column::MaxBlockNumber])
                .value(Column::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .exec(db)
        .await?;

    Ok(())
}
