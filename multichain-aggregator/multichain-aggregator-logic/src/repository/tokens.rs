use crate::{
    repository::{
        batch_update::batch_update,
        macros::{is_distinct_from, update_if_not_null},
        paginate_query,
        pagination::KeySpec,
        prepare_ts_query,
    },
    types::{
        ChainId,
        tokens::{
            AggregatedToken, TokenType, TokenUpdate, UpdateTokenCounters, UpdateTokenMetadata,
            UpdateTokenPriceData, UpdateTokenType,
        },
    },
};
use alloy_primitives::Address;
use entity::tokens::{Column, Entity};
use rust_decimal::Decimal;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, IdenStatic, IntoActiveModel, IntoSimpleExpr,
    JoinType, PartialModelTrait, QueryFilter, QuerySelect, QueryTrait, Select, TransactionError,
    TransactionTrait, prelude::Expr, sea_query::OnConflict,
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
    batch_update(
        db,
        active_models,
        [(Column::UpdatedAt, Expr::current_timestamp().into())],
    )
    .await?;

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
    batch_update(
        db,
        active_models,
        [(Column::UpdatedAt, Expr::current_timestamp().into())],
    )
    .await?;

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
    batch_update(
        db,
        active_models,
        [(Column::UpdatedAt, Expr::current_timestamp().into())],
    )
    .await?;

    Ok(())
}

async fn update_token_type<C>(db: &C, mut updates: Vec<UpdateTokenType>) -> Result<(), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    updates.sort_by(|a, b| (&a.address_hash, a.chain_id).cmp(&(&b.address_hash, b.chain_id)));
    let active_models = updates.into_iter().map(|m| m.into_active_model());
    batch_update(
        db,
        active_models,
        [(Column::UpdatedAt, Expr::current_timestamp().into())],
    )
    .await?;

    Ok(())
}

// Select query for nested `AggregatedToken` struct
// This query converts a single-chain token into a multichain format
pub fn base_normal_tokens_query(
    addresses: Vec<Address>,
    chain_ids: Vec<ChainId>,
    token_types: Vec<TokenType>,
    search_query: Option<String>,
) -> Select<Entity> {
    let addresses_rel = Entity::belongs_to(entity::addresses::Entity)
        .from((Column::AddressHash, Column::ChainId))
        .to((
            entity::addresses::Column::Hash,
            entity::addresses::Column::ChainId,
        ))
        .into();

    Entity::find()
        .join(JoinType::LeftJoin, addresses_rel)
        .apply_if(
            (!chain_ids.is_empty()).then_some(chain_ids),
            |q, chain_ids| q.filter(Column::ChainId.is_in(chain_ids)),
        )
        .apply_if(
            (!token_types.is_empty()).then_some(token_types),
            |q, token_types| q.filter(Column::TokenType.is_in(token_types)),
        )
        .apply_if(search_query, |q, search_query| {
            let ts_query = prepare_ts_query(&search_query);
            q.filter(Expr::cust_with_expr(
                "to_tsvector('english', symbol || ' ' || name) @@ to_tsquery($1)",
                ts_query,
            ))
        })
        .apply_if(
            (!addresses.is_empty()).then_some(addresses),
            |q, addresses| {
                q.filter(
                    Column::AddressHash.is_in(
                        addresses
                            .into_iter()
                            .map(|a| a.to_vec())
                            .collect::<Vec<_>>(),
                    ),
                )
            },
        )
        .select_only()
}

pub async fn get_aggregated_token<C>(
    db: &C,
    address_hash: alloy_primitives::Address,
    chain_id: ChainId,
) -> Result<Option<AggregatedToken>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    let token = base_normal_tokens_query(vec![address_hash], vec![chain_id], vec![], None)
        .into_partial_model::<AggregatedToken>()
        .one(db)
        .await?;

    Ok(token)
}

pub type ListClusterTokensPageToken = (
    Option<Decimal>,
    Option<Decimal>,
    Option<i64>,
    Option<String>,
    Address,
    ChainId,
);

pub async fn list_aggregated_tokens<C>(
    db: &C,
    addresses: Vec<Address>,
    chain_ids: Vec<ChainId>,
    token_types: Vec<TokenType>,
    query: Option<String>,
    page_size: u64,
    page_token: Option<ListClusterTokensPageToken>,
) -> Result<(Vec<AggregatedToken>, Option<ListClusterTokensPageToken>), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    let query = AggregatedToken::select_cols(base_normal_tokens_query(
        addresses,
        chain_ids,
        token_types,
        query,
    ))
    .as_query()
    .to_owned();

    let order_keys = vec![
        KeySpec::desc_nulls_last(Column::CirculatingMarketCap.into_simple_expr()),
        KeySpec::desc_nulls_last(Column::FiatValue.into_simple_expr()),
        KeySpec::desc_nulls_last(Column::HoldersCount.into_simple_expr()),
        KeySpec::asc(Column::Name.into_simple_expr()),
        KeySpec::asc(Column::AddressHash.into_simple_expr()),
        KeySpec::asc(Column::ChainId.into_simple_expr()),
    ];
    let page_token = page_token.map(|(m, f, h, n, a, c)| (m, f, h, n, a.to_vec(), c));

    paginate_query(
        db,
        query,
        page_size,
        page_token,
        order_keys,
        |a: &AggregatedToken| {
            (
                a.circulating_market_cap,
                a.fiat_value,
                a.holders_count,
                a.name.clone(),
                *a.address_hash,
                a.chain_info.chain_id,
            )
        },
    )
    .await
}
