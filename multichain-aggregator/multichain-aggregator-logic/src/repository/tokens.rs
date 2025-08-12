use crate::{
    repository::{
        batch_update::batch_update,
        macros::{col, expr_as, is_distinct_from, map_col, update_if_not_null},
    },
    types::tokens::{
        TokenType, TokenUpdate, UpdateTokenCounters, UpdateTokenMetadata, UpdateTokenPriceData,
        UpdateTokenType, token_chain_infos_expr,
    },
};
use entity::tokens::{Column, Entity};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, IdenStatic, IntoActiveModel,
    TransactionError, TransactionTrait,
    prelude::Expr,
    sea_query::{
        IntoColumnRef, IntoIden, IntoTableRef, OnConflict, Query, SelectExpr, SelectStatement,
        SimpleExpr,
    },
};

pub async fn upsert_many<C>(db: &C, tokens: Vec<TokenUpdate>) -> Result<(), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    let mut metadata_upserts = Vec::new();
    let mut metadata_updates = Vec::new();
    let mut price_updates = Vec::new();
    let mut counter_updates = Vec::new();
    let mut type_updates = Vec::new();

    for token in tokens {
        if let Some(metadata) = token.metadata {
            // Models with present NOT NULL columns are upserted.
            // Other models are optionally updated.
            if metadata.token_type.is_some() {
                metadata_upserts.push(metadata);
            } else {
                metadata_updates.push(metadata);
            }
        }
        if let Some(price_data) = token.price_data {
            price_updates.push(price_data);
        }
        if let Some(counters) = token.counters {
            counter_updates.push(counters);
        }
        if let Some(r#type) = token.r#type {
            type_updates.push(r#type);
        }
    }

    // Process all updates in a single transaction.
    // Only metadata updates can create new tokens,
    // while price and counter updates can only update existing ones.
    db.transaction(|tx| {
        Box::pin(async move {
            if !metadata_upserts.is_empty() {
                upsert_token_metadata(tx, metadata_upserts).await?;
            }

            if !metadata_updates.is_empty() {
                update_token_metadata(tx, metadata_updates).await?;
            }

            if !price_updates.is_empty() {
                update_token_price_data(tx, price_updates).await?;
            }

            if !counter_updates.is_empty() {
                update_token_counters(tx, counter_updates).await?;
            }

            if !type_updates.is_empty() {
                update_token_type(tx, type_updates).await?;
            }

            Ok(())
        })
    })
    .await
    .map_err(|err| match err {
        TransactionError::Connection(e) => e,
        TransactionError::Transaction(e) => e,
    })
}

async fn upsert_token_metadata<C>(
    db: &C,
    mut updates: Vec<UpdateTokenMetadata>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    updates.sort_by(|a, b| (&a.address_hash, a.chain_id).cmp(&(&b.address_hash, b.chain_id)));
    let on_conflict = OnConflict::columns([Column::AddressHash, Column::ChainId])
        .values([
            update_if_not_null!(Column::Name),
            update_if_not_null!(Column::Symbol),
            update_if_not_null!(Column::Decimals),
            update_if_not_null!(Column::TokenType),
            update_if_not_null!(Column::IconUrl),
            update_if_not_null!(Column::TotalSupply),
        ])
        .value(Column::UpdatedAt, Expr::current_timestamp())
        .action_and_where(is_distinct_from!(
            Column::Name,
            Column::Symbol,
            Column::Decimals,
            Column::TokenType,
            Column::IconUrl,
            Column::TotalSupply
        ))
        .to_owned();
    let active_models = updates.into_iter().map(|m| m.into_active_model());
    Entity::insert_many(active_models)
        .on_conflict(on_conflict)
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}

async fn update_token_metadata<C>(
    db: &C,
    mut updates: Vec<UpdateTokenMetadata>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    updates.sort_by(|a, b| (&a.address_hash, a.chain_id).cmp(&(&b.address_hash, b.chain_id)));
    let active_models = updates.into_iter().map(|m| m.into_active_model());
    batch_update(db, active_models).await?;

    Ok(())
}

async fn update_token_price_data<C>(
    db: &C,
    mut updates: Vec<UpdateTokenPriceData>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    updates.sort_by(|a, b| (&a.address_hash, a.chain_id).cmp(&(&b.address_hash, b.chain_id)));
    let active_models = updates.into_iter().map(|m| m.into_active_model());
    batch_update(db, active_models).await?;

    Ok(())
}

async fn update_token_counters<C>(
    db: &C,
    mut updates: Vec<UpdateTokenCounters>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    updates.sort_by(|a, b| (&a.address_hash, a.chain_id).cmp(&(&b.address_hash, b.chain_id)));
    let active_models = updates.into_iter().map(|m| m.into_active_model());
    batch_update(db, active_models).await?;

    Ok(())
}

async fn update_token_type<C>(db: &C, mut updates: Vec<UpdateTokenType>) -> Result<(), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    updates.sort_by(|a, b| (&a.address_hash, a.chain_id).cmp(&(&b.address_hash, b.chain_id)));
    let active_models = updates.into_iter().map(|m| m.into_active_model());
    batch_update(db, active_models).await?;

    Ok(())
}

// Select query for `AggregatedToken` struct
// This query aggregates multiple `ERC-7802` token instances into a single struct.
// NOTE: `name`, `symbol`, `decimals` are assumed to be the same
pub fn aggregated_tokens_query(from: impl IntoTableRef) -> SelectStatement {
    Query::select()
        .exprs([
            col!("token_address_hash"),
            col!("token_name"),
            col!("token_symbol"),
            col!("token_decimals"),
            expr_as!(Expr::val(TokenType::Erc7802), "token_token_type"),
            map_col!("MAX($1)", "token_icon_url"),
            map_col!("AVG($1)", "token_fiat_value"),
            map_col!("AVG($1)", "token_circulating_market_cap"),
            map_col!("SUM($1)", "token_total_supply"),
            map_col!("SUM($1)", "token_holders_count"),
            map_col!("SUM($1)", "token_transfers_count"),
            expr_as!(
                Expr::cust_with_expr("jsonb_agg($1)", token_chain_infos_expr()),
                "token_chain_infos"
            ),
        ])
        .from(from)
        .and_where(Expr::col("token_token_type").eq(TokenType::Erc7802))
        .group_by_columns([
            "token_address_hash",
            "token_name",
            "token_symbol",
            "token_decimals",
        ])
        .to_owned()
}

// Select query for nested `AggregatedToken` struct
// This query converts a single-chain token into a multichain format
pub fn normal_tokens_query(from: impl IntoTableRef) -> SelectStatement {
    Query::select()
        .exprs([
            col!("token_address_hash"),
            col!("token_name"),
            col!("token_symbol"),
            col!("token_decimals"),
            col!("token_token_type"),
            col!("token_icon_url"),
            col!("token_fiat_value"),
            col!("token_circulating_market_cap"),
            col!("token_total_supply"),
            col!("token_holders_count"),
            col!("token_transfers_count"),
            expr_as!(
                Expr::cust_with_expr("jsonb_build_array($1)", token_chain_infos_expr()),
                "token_chain_infos"
            ),
        ])
        .from(from)
        .and_where(Expr::col("token_token_type").ne(TokenType::Erc7802))
        .to_owned()
}
