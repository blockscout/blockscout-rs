use crate::types::address_token_balances::AddressTokenBalance;
use entity::address_token_balances::{ActiveModel, Column, Entity};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, prelude::Expr, sea_query::OnConflict,
};

pub async fn upsert_many<C>(
    db: &C,
    mut address_token_balances: Vec<AddressTokenBalance>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    address_token_balances.sort_by(|a, b| {
        (
            &a.address_hash,
            &a.chain_id,
            &a.token_address_hash,
            &a.token_id,
        )
            .cmp(&(
                &b.address_hash,
                &b.chain_id,
                &b.token_address_hash,
                &b.token_id,
            ))
    });
    let address_token_balances = address_token_balances.into_iter().map(ActiveModel::from);

    Entity::insert_many(address_token_balances)
        .on_conflict(
            OnConflict::new()
                .exprs([
                    Expr::col(Column::AddressHash),
                    Expr::col(Column::ChainId),
                    Expr::col(Column::TokenAddressHash),
                    Expr::expr(Expr::cust_with_expr(
                        "COALESCE($1, -1)",
                        Column::TokenId.into_expr(),
                    )),
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
