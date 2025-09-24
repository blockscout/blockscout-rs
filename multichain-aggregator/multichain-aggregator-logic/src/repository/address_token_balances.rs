use crate::{
    repository::{paginate_query, pagination::KeySpec, tokens::base_normal_tokens_query},
    types::{
        ChainId,
        address_token_balances::{
            AddressTokenBalance, AggregatedAddressTokenBalance, TokenHolder, fiat_balance_query,
        },
    },
};
use bigdecimal::BigDecimal;
use entity::{
    address_token_balances::{ActiveModel, Column, Entity},
    sea_orm_active_enums::TokenType,
    tokens,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, JoinType, PartialModelTrait, QueryFilter,
    QuerySelect, QueryTrait,
    prelude::Expr,
    sea_query::{OnConflict, Query},
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
    token_types: Vec<TokenType>,
    chain_ids: Vec<i64>,
    page_size: u64,
    page_token: Option<ListAddressTokensPageToken>,
) -> Result<
    (
        Vec<AggregatedAddressTokenBalance>,
        Option<ListAddressTokensPageToken>,
    ),
    DbErr,
>
where
    C: ConnectionTrait,
{
    let tokens_rel = tokens::Entity::belongs_to(Entity)
        .from((tokens::Column::AddressHash, tokens::Column::ChainId))
        .to((Column::TokenAddressHash, Column::ChainId))
        .into();

    let query = AggregatedAddressTokenBalance::select_cols(
        base_normal_tokens_query(vec![], chain_ids, token_types, None)
            .join(JoinType::InnerJoin, tokens_rel)
            .filter(Column::AddressHash.eq(address.as_slice()))
            .filter(Column::Value.gt(0)),
    )
    .as_query()
    .to_owned();

    let order_keys = vec![
        KeySpec::desc_nulls_last(fiat_balance_query()),
        KeySpec::desc(Expr::col(Column::Value).into()),
        KeySpec::desc(Expr::col(Column::Id).into()),
    ];

    paginate_query(
        db,
        query,
        page_size,
        page_token,
        order_keys,
        |a: &AggregatedAddressTokenBalance| (a.fiat_balance.clone(), a.value.clone(), a.id),
    )
    .await
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

pub type ListTokenHoldersPageToken = (BigDecimal, alloy_primitives::Address);

pub async fn list_token_holders<C>(
    db: &C,
    address: alloy_primitives::Address,
    chain_id: ChainId,
    page_size: u64,
    page_token: Option<ListTokenHoldersPageToken>,
) -> Result<(Vec<TokenHolder>, Option<ListTokenHoldersPageToken>), DbErr>
where
    C: ConnectionTrait,
{
    let query = Entity::find()
        .filter(Column::TokenAddressHash.eq(address.as_slice()))
        .filter(Column::ChainId.eq(chain_id))
        .filter(Column::Value.gt(0))
        .as_query()
        .to_owned();

    let order_keys = vec![
        KeySpec::desc(Expr::col(Column::Value).into()),
        KeySpec::desc(Expr::col(Column::AddressHash).into()),
    ];
    let page_token = page_token.map(|(v, a)| (v, a.to_vec()));

    paginate_query(
        db,
        query,
        page_size,
        page_token,
        order_keys,
        |a: &TokenHolder| (a.value.clone(), *a.address_hash),
    )
    .await
}
