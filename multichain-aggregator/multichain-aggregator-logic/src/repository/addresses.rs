use crate::{
    error::{ParseError, ServiceError},
    types::addresses::Address,
};
use entity::addresses::{ActiveModel, Column, Entity, Model};
use regex::Regex;
use sea_orm::{
    prelude::Expr, sea_query::OnConflict, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, DbErr,
    EntityTrait, Iterable, QueryFilter, QuerySelect,
};
use std::sync::OnceLock;

fn words_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[a-zA-Z0-9]+").unwrap())
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

pub async fn find_by_address<C>(
    db: &C,
    address: alloy_primitives::Address,
) -> Result<Vec<Address>, ServiceError>
where
    C: ConnectionTrait,
{
    let res = Entity::find()
        .filter(Column::Hash.eq(address.as_slice()))
        .all(db)
        .await?
        .into_iter()
        .map(Address::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(res)
}

pub async fn search_by_query<C>(db: &C, q: &str) -> Result<Vec<Address>, ServiceError>
where
    C: ConnectionTrait,
{
    let mut query = Entity::find();

    if let Ok(address) = try_parse_address(q) {
        query = query.filter(Column::Hash.eq(address.as_slice()));
    } else {
        let ts_query = prepare_ts_query(q);
        query = query.filter(Expr::cust_with_expr(
            "to_tsvector('english', contract_name) @@ to_tsquery($1) OR \
                to_tsvector('english', ens_name) @@ to_tsquery($1) OR \
                to_tsvector('english', token_name) @@ to_tsquery($1)",
            ts_query,
        ));
    }

    let res = query
        .limit(50)
        .all(db)
        .await?
        .into_iter()
        .map(Address::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(res)
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
