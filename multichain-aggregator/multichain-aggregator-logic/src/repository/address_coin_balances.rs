use crate::types::address_coin_balances::AddressCoinBalance;
use entity::address_coin_balances::{ActiveModel, Column, Entity};
use sea_orm::{prelude::Expr, sea_query::OnConflict, ConnectionTrait, DbErr, EntityTrait};

pub async fn upsert_many<C>(
    db: &C,
    mut address_coin_balances: Vec<AddressCoinBalance>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    address_coin_balances
        .sort_by(|a, b| (&a.address_hash, a.chain_id).cmp(&(&b.address_hash, b.chain_id)));
    let address_coin_balances = address_coin_balances.into_iter().map(ActiveModel::from);

    Entity::insert_many(address_coin_balances)
        .on_conflict(
            OnConflict::columns([Column::AddressHash, Column::ChainId])
                .update_columns([Column::Value])
                .value(Column::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}
