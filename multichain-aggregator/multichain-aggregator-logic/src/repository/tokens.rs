use crate::{
    repository::macros::{is_distinct_from, update_if_not_null},
    types::tokens::{TokenUpdate, UpdateTokenCounters, UpdateTokenMetadata, UpdateTokenPriceData},
};
use entity::tokens::{Column, Entity};
use sea_orm::{
    prelude::Expr,
    sea_query::{Alias, ColumnRef, CommonTableExpression, IntoIden, OnConflict, Query, ValueTuple},
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DbErr, EntityName, EntityTrait,
    IdenStatic, IntoActiveModel, IntoSimpleExpr, Iterable, PrimaryKeyToColumn, TransactionError,
    TransactionTrait,
};

pub async fn upsert_many<C>(db: &C, tokens: Vec<TokenUpdate>) -> Result<(), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    let mut metadata_updates = Vec::new();
    let mut price_updates = Vec::new();
    let mut counter_updates = Vec::new();

    for token in tokens {
        match token {
            TokenUpdate::Metadata(metadata) => metadata_updates.push(metadata),
            TokenUpdate::PriceData(price_data) => price_updates.push(price_data),
            TokenUpdate::Counters(counters) => counter_updates.push(counters),
        }
    }

    // Process all updates in a single transaction.
    // Only metadata updates can create new tokens,
    // while price and counter updates can only update existing ones.
    db.transaction(|tx| {
        Box::pin(async move {
            if !metadata_updates.is_empty() {
                upsert_token_metadata(tx, metadata_updates).await?;
            }

            if !price_updates.is_empty() {
                update_token_price_data(tx, price_updates).await?;
            }

            if !counter_updates.is_empty() {
                update_token_counters(tx, counter_updates).await?;
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
        vec![Column::FiatValue, Column::CirculatingMarketCap],
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
        vec![Column::TransfersCount, Column::HoldersCount],
    )
    .await?;

    Ok(())
}

async fn batch_update<C, A>(
    db: &C,
    models: impl IntoIterator<Item = A>,
    update_columns: Vec<<A::Entity as EntityTrait>::Column>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
    A: ActiveModelTrait,
{
    let cte_name = Alias::new("updates").into_iden();

    // Select all the columns that are part of the primary key,
    // as well as the columns that are explicitly being updated.
    let mut required_columns = <A::Entity as EntityTrait>::PrimaryKey::iter()
        .map(|c| c.into_column())
        .collect::<Vec<_>>();
    required_columns.extend(update_columns.clone());

    // Get all values for required columns for each model
    let value_tuples = models
        .into_iter()
        .map(|m| {
            required_columns
                .iter()
                .map(|c| {
                    m.get(*c)
                        .into_value()
                        .ok_or_else(|| DbErr::Custom(format!("missing column: {c:?}")))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(ValueTuple::Many)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let cte = CommonTableExpression::new()
        .query(
            Query::select()
                .column(ColumnRef::Asterisk)
                .from_values(value_tuples, cte_name.clone())
                .to_owned(),
        )
        .table_name(cte_name.clone())
        .columns(required_columns)
        .to_owned();

    // Map table columns to CTE columns
    let update_columns_mapping = update_columns
        .iter()
        .map(|c| (*c, Expr::col((cte_name.clone(), *c)).into_simple_expr()));

    // Match rows from CTE with rows from the table by primary key
    let mut conditions = Condition::all();
    for key in <A::Entity as EntityTrait>::PrimaryKey::iter() {
        let col = key.into_column();
        let cte_col = Expr::col((cte_name.clone(), col));
        let table_col = col.into_simple_expr();
        conditions = conditions.add(table_col.eq(cte_col));
    }

    let query = Query::update()
        .table(A::Entity::default().table_ref())
        .values(update_columns_mapping)
        .with_cte(cte)
        .from(cte_name)
        .cond_where(conditions)
        .to_owned();

    let stmt = db.get_database_backend().build(&query);
    db.execute(stmt).await?;

    Ok(())
}
