use crate::types::address_token_balances::AddressTokenBalance;
use entity::address_token_balances::{ActiveModel, Column, Entity};
use sea_orm::{prelude::Expr, sea_query::OnConflict, ConnectionTrait, DbErr, EntityTrait};

pub async fn upsert_many<C>(
    db: &C,
    mut address_token_balances: Vec<AddressTokenBalance>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    address_token_balances.sort_by(|a, b| {
        (&a.address_hash, &a.chain_id, &a.token_address_hash).cmp(&(
            &b.address_hash,
            &b.chain_id,
            &b.token_address_hash,
        ))
    });
    let address_token_balances = address_token_balances.into_iter().map(ActiveModel::from);

    Entity::insert_many(address_token_balances)
        .on_conflict(
            OnConflict::columns([
                Column::AddressHash,
                Column::ChainId,
                Column::TokenAddressHash,
            ])
            .update_columns([Column::Value])
            .value(Column::UpdatedAt, Expr::current_timestamp())
            .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}
