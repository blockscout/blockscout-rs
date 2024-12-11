use crate::{
    error::{ParseError, ServiceError},
    types::{addresses::Address, ChainId},
};
use alloy_primitives::Address as AddressAlloy;
use entity::addresses::{ActiveModel, Column, Entity, Model};
use regex::Regex;
use sea_orm::{
    prelude::Expr,
    sea_query::{OnConflict, PostgresQueryBuilder},
    ActiveValue::NotSet,
    ConnectionTrait, DbErr, EntityTrait, IntoSimpleExpr, Iterable, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait,
};
use std::sync::OnceLock;

fn words_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[a-zA-Z0-9]+").unwrap())
}

fn hex_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(0x)?[0-9a-fA-F]{3,40}$").unwrap())
}

pub async fn upsert_many<C>(db: &C, addresses: Vec<Address>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    if addresses.is_empty() {
        return Ok(());
    }

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
        .exec(db)
        .await?;

    Ok(())
}

pub async fn search_by_query<C>(db: &C, q: &str) -> Result<Vec<Address>, ServiceError>
where
    C: ConnectionTrait,
{
    search_by_query_paginated(db, q, None, 100)
        .await
        .map(|(addresses, _)| addresses)
}

pub async fn search_by_query_paginated<C>(
    db: &C,
    q: &str,
    page_token: Option<(ChainId, AddressAlloy)>,
    limit: u64,
) -> Result<(Vec<Address>, Option<(ChainId, AddressAlloy)>), ServiceError>
where
    C: ConnectionTrait,
{
    let page_token = page_token.unwrap_or((ChainId::MIN, AddressAlloy::ZERO));
    let mut query = Entity::find()
        .filter(
            Expr::tuple([
                Column::ChainId.into_simple_expr(),
                Column::Hash.into_simple_expr(),
            ])
            .gte(Expr::tuple([
                page_token.0.into(),
                page_token.1.as_slice().into(),
            ])),
        )
        .order_by_asc(Column::Hash)
        .order_by_asc(Column::ChainId)
        .limit(limit + 1);

    if hex_regex().is_match(q) {
        let prefix = format!("\\x{}", q.strip_prefix("0x").unwrap_or(q));
        query = query.filter(Expr::cust_with_expr("hash LIKE $1", format!("{}%", prefix)));
    } else {
        let ts_query = prepare_ts_query(q);
        query = query.filter(Expr::cust_with_expr(
            "to_tsvector('english', contract_name) @@ to_tsquery($1) OR \
                to_tsvector('english', ens_name) @@ to_tsquery($1) OR \
                to_tsvector('english', token_name) @@ to_tsquery($1)",
            ts_query,
        ));
    }

    println!("{}", query.as_query().to_string(PostgresQueryBuilder));

    let addresses = query
        .all(db)
        .await?
        .into_iter()
        .map(Address::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    match addresses.get(limit as usize) {
        Some(a) => Ok((
            addresses[0..limit as usize].to_vec(),
            Some((a.chain_id, a.hash)),
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

pub fn try_parse_address(query: &str) -> Result<alloy_primitives::Address, ParseError> {
    query.parse().map_err(ParseError::from)
}

fn prepare_ts_query(query: &str) -> String {
    words_regex()
        .find_iter(query.trim())
        .map(|w| format!("{}:*", w.as_str()))
        .collect::<Vec<String>>()
        .join(" & ")
}
