use crate::types::{ChainId, chains::Chain};
use entity::{
    api_keys,
    chains::{ActiveModel, Column, Entity, Model},
};
use sea_orm::{
    ActiveValue::NotSet, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, prelude::Expr, sea_query::OnConflict,
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
                .update_columns([Column::ExplorerUrl, Column::IconUrl, Column::Name])
                .value(Column::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}

pub async fn list_by_ids<C>(db: &C, ids: Vec<ChainId>) -> Result<Vec<Model>, DbErr>
where
    C: ConnectionTrait,
{
    Entity::find()
        .order_by_asc(Column::Id)
        .filter(Column::Id.is_in(ids))
        .all(db)
        .await
}

pub async fn list_by_active_api_keys<C>(
    db: &C,
    with_active_api_keys: bool,
) -> Result<Vec<Model>, DbErr>
where
    C: ConnectionTrait,
{
    let mut query = Entity::find().order_by_asc(Column::Id);
    if with_active_api_keys {
        // Filter out chains without active api keys
        query = query.distinct_on([Column::Id]).inner_join(api_keys::Entity);
    }
    query.all(db).await
}
