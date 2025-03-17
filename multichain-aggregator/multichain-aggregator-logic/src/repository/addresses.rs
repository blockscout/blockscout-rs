use crate::types::{addresses::Address, ChainId};
use alloy_primitives::Address as AddressAlloy;
use entity::{
    addresses::{ActiveModel, Column, Entity, Model},
    sea_orm_active_enums as db_enum,
};
use regex::Regex;
use sea_orm::{
    prelude::Expr,
    sea_query::{Alias, ColumnRef, CommonTableExpression, OnConflict, Query, WithClause},
    ActiveValue::NotSet,
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, FromQueryResult, IntoSimpleExpr, Iterable,
    Order, QuerySelect,
};
use std::sync::OnceLock;

fn words_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[a-zA-Z0-9]+").unwrap())
}

pub async fn upsert_many<C>(db: &C, mut addresses: Vec<Address>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    addresses.sort_by(|a, b| (a.hash, a.chain_id).cmp(&(b.hash, b.chain_id)));
    let addresses = addresses.into_iter().map(|address| {
        let model: Model = address.into();
        let mut active: ActiveModel = model.into();
        active.created_at = NotSet;
        active.updated_at = NotSet;
        active
    });

    Entity::insert_many(addresses)
        .on_conflict(
            OnConflict::columns([Column::Hash, Column::ChainId])
                .update_columns(non_primary_columns())
                .value(Column::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}

pub async fn list<C>(
    db: &C,
    address: Option<AddressAlloy>,
    query: Option<String>,
    chain_id: Option<ChainId>,
    token_types: Option<Vec<db_enum::TokenType>>,
    page_size: u64,
    page_token: Option<(AddressAlloy, ChainId)>,
) -> Result<(Vec<Model>, Option<(AddressAlloy, ChainId)>), DbErr>
where
    C: ConnectionTrait,
{
    // Materialize addresses CTE when searching by contract_name.
    // Otherwise, query planner chooses a suboptimal plan.
    let is_cte_materialized = query.is_some();
    let addresses_cte = CommonTableExpression::new()
        .query(
            QuerySelect::query(&mut Entity::find())
                .apply_if(query, |q, query| {
                    let ts_query = prepare_ts_query(&query);
                    q.and_where(Expr::cust_with_expr(
                        "to_tsvector('english', contract_name) @@ to_tsquery($1)",
                        ts_query,
                    ));
                })
                .to_owned(),
        )
        .materialized(is_cte_materialized)
        .table_name(Alias::new("addresses"))
        .to_owned();

    let base_select = Query::select()
        .column(ColumnRef::Asterisk)
        .from(Alias::new("addresses")) // NOTE: this is the CTE reference
        .apply_if(chain_id, |q, chain_id| {
            q.and_where(Column::ChainId.eq(chain_id));
        })
        .apply_if(address, |q, address| {
            q.and_where(Column::Hash.eq(address.as_slice()));
        })
        .apply_if(token_types, |q, token_types| {
            q.and_where(Column::TokenType.is_in(token_types));
        })
        .apply_if(page_token, |q, page_token| {
            q.and_where(
                Expr::tuple([
                    Column::Hash.into_simple_expr(),
                    Column::ChainId.into_simple_expr(),
                ])
                .gte(Expr::tuple([
                    page_token.0.as_slice().into(),
                    page_token.1.into(),
                ])),
            );
        })
        .order_by_columns(vec![
            (Column::Hash, Order::Asc),
            (Column::ChainId, Order::Asc),
        ])
        .limit(page_size + 1)
        .to_owned();

    let query = WithClause::new()
        .cte(addresses_cte)
        .to_owned()
        .query(base_select);

    let addresses = Model::find_by_statement(db.get_database_backend().build(&query))
        .all(db)
        .await?;

    match addresses.get(page_size as usize) {
        Some(a) => Ok((
            addresses[..page_size as usize].to_vec(),
            Some((
                // unwrap is safe here because addresses are validated prior to being inserted
                AddressAlloy::try_from(a.hash.as_slice()).unwrap(),
                a.chain_id,
            )),
        )),
        None => Ok((addresses, None)),
    }
}

fn non_primary_columns() -> impl Iterator<Item = Column> {
    Column::iter().filter(|col| {
        !matches!(
            col,
            Column::Hash | Column::ChainId | Column::CreatedAt | Column::UpdatedAt
        )
    })
}

fn prepare_ts_query(query: &str) -> String {
    words_regex()
        .find_iter(query.trim())
        .map(|w| format!("{}:*", w.as_str()))
        .collect::<Vec<String>>()
        .join(" & ")
}
