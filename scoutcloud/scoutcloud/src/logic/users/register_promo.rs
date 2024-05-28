use scoutcloud_entity::register_promo;
use sea_orm::prelude::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PromoError {
    #[error("Promo code not found")]
    NotFound,
    #[error("Promo code already used")]
    AlreadyUsed,
    #[error("Db error: {0}")]
    Db(#[from] DbErr),
}

pub async fn try_use_promo<C: ConnectionTrait>(
    db: &C,
    code: &str,
) -> Result<register_promo::Model, PromoError> {
    let promo = default_select()
        .filter(register_promo::Column::Code.eq(code))
        .one(db)
        .await?
        .ok_or(PromoError::NotFound)?;

    if promo.max_activations > 0 {
        let activations = scoutcloud_entity::balance_changes::Entity::find()
            .filter(scoutcloud_entity::balance_changes::Column::RegisterPromoId.eq(promo.id))
            .count(db)
            .await?;
        if activations >= promo.max_activations as u64 {
            return Err(PromoError::AlreadyUsed);
        }
    }
    Ok(promo)
}

fn default_select() -> Select<register_promo::Entity> {
    register_promo::Entity::find().filter(register_promo::Column::Deleted.eq(false))
}
