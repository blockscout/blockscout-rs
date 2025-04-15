use crate::types::{addresses::Address, ChainId};
use alloy_primitives::Address as AddressAlloy;
use entity::{
    addresses::{ActiveModel, Column, Entity, Model},
    sea_orm_active_enums as db_enum,
};
use regex::Regex;
use sea_orm::{
    prelude::Expr,
    sea_query::{
        Alias, ColumnRef, CommonTableExpression, IntoIden, OnConflict, Query, WindowStatement,
        WithClause,
    },
    ActiveValue::NotSet,
    ColumnTrait, ConnectionTrait, DbErr, EntityName, EntityTrait, FromQueryResult, IntoSimpleExpr,
    Iterable, Order, QueryFilter, QuerySelect,
};
use std::{collections::HashMap, sync::OnceLock};

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

fn prepare_address_search_cte(
    contract_name_query: Option<String>,
    cte_name: impl IntoIden,
) -> CommonTableExpression {
    // Materialize addresses CTE when searching by contract_name.
    // Otherwise, query planner chooses a suboptimal plan.
    // If query is not provided, this CTE will be folded by the optimizer.
    let is_cte_materialized = contract_name_query.is_some();

    CommonTableExpression::new()
        .query(
            Query::select()
                .column(ColumnRef::Asterisk)
                .from(Entity.table_ref())
                .apply_if(contract_name_query, |q, query| {
                    let ts_query = prepare_ts_query(&query);
                    q.and_where(Expr::cust_with_expr(
                        "to_tsvector('english', contract_name) @@ to_tsquery($1)",
                        ts_query,
                    ));
                })
                // Apply a hard limit in case we materialize the CTE
                .apply_if(is_cte_materialized.then_some(10_000), |q, limit| {
                    q.limit(limit);
                })
                .to_owned(),
        )
        .materialized(is_cte_materialized)
        .table_name(cte_name)
        .to_owned()
}

pub async fn uniform_chain_search<C>(
    db: &C,
    contract_name_query: String,
    token_types: Option<Vec<db_enum::TokenType>>,
    chain_ids: Vec<ChainId>,
) -> Result<Vec<Model>, DbErr>
where
    C: ConnectionTrait,
{
    if chain_ids.is_empty() {
        return Ok(vec![]);
    }

    let ts_rank_ordering = Expr::cust_with_expr(
        "ts_rank(to_tsvector('english', contract_name), to_tsquery($1))",
        prepare_ts_query(&contract_name_query),
    );

    let addresses_cte_iden = Alias::new("addresses").into_iden();
    let addresses_cte =
        prepare_address_search_cte(Some(contract_name_query), addresses_cte_iden.clone());

    let row_number = Expr::custom_keyword(Alias::new("ROW_NUMBER()"));
    let ranked_addresses_iden = Alias::new("ranked_addresses").into_iden();
    let ranked_addresses_cte = CommonTableExpression::new()
        .query(
            Query::select()
                .column(ColumnRef::TableAsterisk(addresses_cte_iden.clone()))
                .expr_window_as(
                    row_number,
                    WindowStatement::partition_by(Column::ChainId)
                        .order_by_expr(ts_rank_ordering, Order::Desc)
                        .order_by(Column::Hash, Order::Asc)
                        .to_owned(),
                    Alias::new("rn"),
                )
                .from(addresses_cte_iden.clone())
                .and_where(Column::ChainId.is_in(chain_ids.clone()))
                .apply_if(token_types, |q, token_types| {
                    if !token_types.is_empty() {
                        q.and_where(Column::TokenType.is_in(token_types));
                    } else {
                        q.and_where(Column::TokenType.is_null());
                    }
                })
                .to_owned(),
        )
        .table_name(ranked_addresses_iden.clone())
        .to_owned();

    let limit = chain_ids.len() as u64;
    let base_select = Query::select()
        .column(ColumnRef::Asterisk)
        .from(ranked_addresses_iden)
        .and_where(Expr::col(Alias::new("rn")).eq(1))
        .order_by_expr(
            Expr::cust_with_exprs(
                "array_position($1, $2)",
                [chain_ids.into(), Expr::col(Column::ChainId).into()],
            ),
            Order::Asc,
        )
        .limit(limit)
        .to_owned();

    let query = WithClause::new()
        .cte(addresses_cte)
        .cte(ranked_addresses_cte)
        .to_owned()
        .query(base_select);

    let addresses = Model::find_by_statement(db.get_database_backend().build(&query))
        .all(db)
        .await?;

    Ok(addresses)
}

pub async fn get_batch<C>(
    db: &C,
    pks: Vec<(&AddressAlloy, ChainId)>,
) -> Result<HashMap<(AddressAlloy, ChainId), Model>, DbErr>
where
    C: ConnectionTrait,
{
    let models = Entity::find()
        .filter(
            Expr::tuple([
                Column::Hash.into_simple_expr(),
                Column::ChainId.into_simple_expr(),
            ])
            .is_in(
                pks.into_iter()
                    .map(|(address, chain_id)| {
                        Expr::tuple([address.as_slice().into(), chain_id.into()]).into_simple_expr()
                    })
                    .collect::<Vec<_>>(),
            ),
        )
        .all(db)
        .await?;

    let pk_to_model = models
        .into_iter()
        .map(|m| {
            let address = parse_db_address(m.hash.as_slice());
            let pk = (address, m.chain_id);
            (pk, m)
        })
        .collect();

    Ok(pk_to_model)
}

pub async fn list<C>(
    db: &C,
    addresses: Vec<AddressAlloy>,
    contract_name_query: Option<String>,
    chain_ids: Vec<ChainId>,
    token_types: Option<Vec<db_enum::TokenType>>,
    page_size: u64,
    page_token: Option<(AddressAlloy, ChainId)>,
) -> Result<(Vec<Model>, Option<(AddressAlloy, ChainId)>), DbErr>
where
    C: ConnectionTrait,
{
    let addresses_cte_iden = Alias::new("addresses").into_iden();
    let addresses_cte = prepare_address_search_cte(contract_name_query, addresses_cte_iden.clone());

    let base_select = QuerySelect::query(&mut Entity::find())
        .from_clear()
        .from(addresses_cte_iden)
        .apply_if(
            (!chain_ids.is_empty()).then_some(chain_ids),
            |q, chain_ids| {
                q.and_where(Column::ChainId.is_in(chain_ids));
            },
        )
        .apply_if(token_types, |q, token_types| {
            if !token_types.is_empty() {
                q.and_where(Column::TokenType.is_in(token_types));
            } else {
                q.and_where(Column::TokenType.is_null());
            }
        })
        .apply_if(
            (!addresses.is_empty()).then_some(addresses),
            |q, addresses| {
                q.and_where(
                    Column::Hash.is_in(
                        addresses
                            .into_iter()
                            .map(|a| a.to_vec())
                            .collect::<Vec<_>>(),
                    ),
                );
            },
        )
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
            Some((parse_db_address(a.hash.as_slice()), a.chain_id)),
        )),
        None => Ok((addresses, None)),
    }
}

fn parse_db_address(hash: &[u8]) -> AddressAlloy {
    AddressAlloy::try_from(hash).expect("db address should be valid")
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
