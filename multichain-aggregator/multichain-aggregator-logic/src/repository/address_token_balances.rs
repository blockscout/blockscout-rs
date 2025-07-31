use crate::types::{
    ChainId,
    address_token_balances::{
        AddressTokenBalance, ExtendedAddressTokenBalance, fiat_balance_query,
    },
};
use bigdecimal::BigDecimal;
use entity::{
    address_token_balances::{ActiveModel, Column, Entity},
    sea_orm_active_enums::TokenType,
    tokens,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, IntoSimpleExpr, JoinType, Order, QueryFilter,
    QueryOrder, QuerySelect, QueryTrait,
    prelude::Expr,
    sea_query::{Alias, Iden, NullOrdering, OnConflict, Query},
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

pub type ListAddressTokensPageToken = (Option<BigDecimal>, BigDecimal, i64);

pub async fn list_by_address<C>(
    db: &C,
    address: alloy_primitives::Address,
    token_type: Option<TokenType>,
    chain_ids: Vec<i64>,
    page_size: u64,
    page_token: Option<ListAddressTokensPageToken>,
) -> Result<
    (
        Vec<ExtendedAddressTokenBalance>,
        Option<ListAddressTokensPageToken>,
    ),
    DbErr,
>
where
    C: ConnectionTrait,
{
    let tokens_rel = Entity::belongs_to(tokens::Entity)
        .from((Column::TokenAddressHash, Column::ChainId))
        .to((tokens::Column::AddressHash, tokens::Column::ChainId))
        .into();

    let fiat_balance_iden = Alias::new("fiat_balance");
    let fiat_balance_col = Expr::col(fiat_balance_iden.clone());

    let balances = Entity::find()
        .expr_as(fiat_balance_query(), fiat_balance_iden.to_string())
        .join(JoinType::InnerJoin, tokens_rel)
        .filter(Column::AddressHash.eq(address.as_slice()))
        .filter(Column::ChainId.is_in(chain_ids))
        .filter(Column::Value.gt(0))
        .apply_if(token_type, |q, token_type| {
            q.filter(tokens::Column::TokenType.eq(token_type))
        })
        .apply_if(page_token, |q, page_token| {
            let (fiat_value, value, id) = page_token;
            match fiat_value {
                None => q.filter(Expr::cust_with_exprs(
                    "$1 IS NULL AND ($2 < $3 OR ($2 = $3 AND $4 < $5))",
                    [
                        fiat_balance_query(),
                        Column::Value.into_simple_expr(),
                        Expr::value(value),
                        Column::Id.into_simple_expr(),
                        Expr::value(id),
                    ],
                )),
                Some(fiat_value) => q.filter(Expr::cust_with_exprs(
                    "$1 < $2 OR $1 IS NULL OR ($1 = $2 AND ($3 < $4 OR ($3 = $4 AND $5 < $6)))",
                    [
                        fiat_balance_query(),
                        Expr::value(fiat_value),
                        Column::Value.into_simple_expr(),
                        Expr::value(value),
                        Column::Id.into_simple_expr(),
                        Expr::value(id),
                    ],
                )),
            }
        })
        .order_by_with_nulls(fiat_balance_col, Order::Desc, NullOrdering::Last)
        .order_by_desc(Column::Value)
        .order_by_desc(Column::Id)
        .limit(page_size + 1)
        .into_partial_model::<ExtendedAddressTokenBalance>()
        .all(db)
        .await?;

    if balances.len() as u64 > page_size {
        Ok((
            balances[..page_size as usize].to_vec(),
            balances
                .get(page_size as usize - 1)
                .map(|a| (a.fiat_balance.clone(), a.value.clone(), a.id)),
        ))
    } else {
        Ok((balances, None))
    }
}

pub async fn check_if_tokens_at_address<C>(
    db: &C,
    address: alloy_primitives::Address,
    cluster_chain_ids: Vec<ChainId>,
) -> Result<bool, DbErr>
where
    C: ConnectionTrait,
{
    let query = Query::select()
        .expr(Expr::exists(
            Query::select()
                .column(Column::Id)
                .from(Entity)
                .and_where(Column::AddressHash.eq(address.as_slice()))
                .and_where(Column::ChainId.is_in(cluster_chain_ids))
                .to_owned(),
        ))
        .to_owned();

    db.query_one(db.get_database_backend().build(&query))
        .await?
        .expect("expr should be present")
        .try_get_by_index(0)
}
