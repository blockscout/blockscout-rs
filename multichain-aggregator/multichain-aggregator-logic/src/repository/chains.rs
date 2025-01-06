use crate::types::chains::Chain;
use entity::chains::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    prelude::Expr, sea_query::OnConflict, ActiveValue::NotSet, ConnectionTrait, DbErr, EntityTrait,
};

pub async fn upsert_many<C>(db: &C, chains: Vec<Chain>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    if chains.is_empty() {
        return Ok(());
    }

    let chains = chains.into_iter().map(|chain| {
        let model: Model = chain.into();
        let mut active: ActiveModel = model.into();
        active.created_at = NotSet;
        active.updated_at = NotSet;
        active
    });

    Entity::insert_many(chains)
        .on_conflict(
            OnConflict::columns([Column::Id])
                .update_columns([Column::ExplorerUrl, Column::IconUrl])
                .value(Column::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}
