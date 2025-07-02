use crate::types::counters::{ChainCounters, TokenCounters};
use entity::{
    counters_token_imported::{
        ActiveModel as CountersTokenImportedActiveModel,
        Column as CountersTokenColumn,
        Entity as CountersTokenImportedModel,
        Model as CountersTokenModel,
    },
    counters_global_imported::{
        ActiveModel as GlobalCountersActiveModel,
        Column as GlobalCountersColumn,
        Entity as GlobalCountersEntity,
    },
};
use sea_orm::{
    prelude::Expr,
    sea_query::OnConflict,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait,
    Iterable, QueryFilter, ActiveModelTrait,
};
use sea_orm::QueryOrder;
use chrono::{NaiveDateTime, NaiveTime, Utc};

pub async fn upsert_token_counters<C>(db: &C, counters: Vec<TokenCounters>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let addresses = counters.into_iter().map(|counter| {
        let model: CountersTokenModel = counter.into();
        let mut active: CountersTokenImportedActiveModel = model.into();
        active.created_at = NotSet;
        active.updated_at = NotSet;
        active
    });

    CountersTokenImportedModel::insert_many(addresses)
        .on_conflict(
            OnConflict::columns([CountersTokenColumn::TokenAddress, CountersTokenColumn::ChainId])
                .update_columns(non_primary_columns_token_counters())
                .value(CountersTokenColumn::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}

fn non_primary_columns_token_counters() -> impl Iterator<Item = CountersTokenColumn> {
    CountersTokenColumn::iter().filter(|col| {
        !matches!(
            col,
            CountersTokenColumn::TokenAddress | CountersTokenColumn::ChainId | CountersTokenColumn::CreatedAt | CountersTokenColumn::UpdatedAt
        )
    })
}

pub async fn upsert_chain_counters<C>(db: &C, data: ChainCounters) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let ChainCounters {
        chain_id,
        timestamp,
        daily_transactions_number,
        total_transactions_number,
        total_addresses_number,
    } = data;

    let day_start = NaiveDateTime::new(timestamp.date(), NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    let day_end = day_start + chrono::Duration::days(1);

    let existing = GlobalCountersEntity::find()
        .filter(GlobalCountersColumn::ChainId.eq(chain_id))
        .filter(GlobalCountersColumn::UpdatedAt.gte(day_start))
        .filter(GlobalCountersColumn::UpdatedAt.lt(day_end))
        .order_by_desc(GlobalCountersColumn::UpdatedAt)
        .one(db)
        .await?;

    match existing {
        Some(model) => {
            let mut active: GlobalCountersActiveModel = model.into();

            if let Some(val) = daily_transactions_number {
                active.daily_transactions_number = Set(Some(val as i64));
            }
            if let Some(val) = total_transactions_number {
                active.total_transactions_number = Set(Some(val as i64));
            }
            if let Some(val) = total_addresses_number {
                active.total_addresses_number = Set(Some(val as i64));
            }

            active.updated_at = Set(Utc::now().naive_utc());
            active.update(db).await?;
        }
        None => {
            let new = GlobalCountersActiveModel {
                id: NotSet,
                chain_id: Set(chain_id),
                daily_transactions_number: Set(daily_transactions_number.map(|v| v as i64)),
                total_transactions_number: Set(total_transactions_number.map(|v| v as i64)),
                total_addresses_number: Set(total_addresses_number.map(|v| v as i64)),
                created_at: Set(Utc::now().naive_utc()),
                updated_at: Set(Utc::now().naive_utc()),
            };
            new.insert(db).await?;
        }
    }

    Ok(())
}