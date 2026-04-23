use crate::types::poor_reputation_tokens::PoorReputationToken;
use entity::poor_reputation_tokens::{ActiveModel, Column, Entity};
use sea_orm::{ConnectionTrait, DbErr, EntityTrait, sea_query::OnConflict};

pub async fn upsert_many<C>(db: &C, mut tokens: Vec<PoorReputationToken>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    if tokens.is_empty() {
        return Ok(());
    }

    tokens.sort_by(|a, b| (&a.address_hash, a.chain_id).cmp(&(&b.address_hash, b.chain_id)));
    let models = tokens.into_iter().map(ActiveModel::from);

    Entity::insert_many(models)
        .on_conflict(
            OnConflict::columns([Column::AddressHash, Column::ChainId])
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}
