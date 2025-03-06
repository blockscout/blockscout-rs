use crate::types::{addresses::Address, ChainId};
use alloy_primitives::Address as AddressAlloy;
use entity::{
    addresses::{ActiveModel, Column, Entity, Model},
    sea_orm_active_enums as db_enum,
};
use regex::Regex;
use sea_orm::{
    prelude::Expr, sea_query::OnConflict, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, DbErr,
    EntityTrait, IntoSimpleExpr, Iterable, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
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
    let addresses = Entity::find()
        .apply_if(chain_id, |q, chain_id| {
            q.filter(Column::ChainId.eq(chain_id))
        })
        .apply_if(address, |q, address| {
            q.filter(Column::Hash.eq(address.as_slice()))
        })
        .apply_if(query, |q, query| {
            let ts_query = prepare_ts_query(&query);
            q.filter(Expr::cust_with_expr(
                "to_tsvector('english', contract_name) @@ to_tsquery($1)",
                ts_query,
            ))
        })
        .apply_if(token_types, |q, token_types| {
            q.filter(Column::TokenType.is_in(token_types))
        })
        .apply_if(page_token, |q, page_token| {
            q.filter(
                Expr::tuple([
                    Column::Hash.into_simple_expr(),
                    Column::ChainId.into_simple_expr(),
                ])
                .gte(Expr::tuple([
                    page_token.0.as_slice().into(),
                    page_token.1.into(),
                ])),
            )
        })
        // Because of ORDER BY (primary_key) and LIMIT clauses, query planner chooses to use pk index
        // instead of specialized indexes on filtered columns, which almost always results in a seqscan
        // because in our case data is sparse (especially for text columns). To prevent it, we wrap
        // the ordered columns in a COALESCE which makes query planner think it is an expression
        // and disregards the primary key index.
        .order_by_asc(Expr::cust_with_exprs(
            "COALESCE($1)",
            [Column::Hash.into_simple_expr()],
        ))
        .order_by_asc(Expr::cust_with_exprs(
            "COALESCE($1)",
            [Column::ChainId.into_simple_expr()],
        ))
        .limit(page_size + 1)
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
