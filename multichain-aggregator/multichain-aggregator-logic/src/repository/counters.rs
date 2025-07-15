use crate::types::counters::ChainCounters;
use chrono::{NaiveDateTime, NaiveTime, Utc};
use entity::counters_global_imported::{
    ActiveModel as GlobalCountersActiveModel, Column as GlobalCountersColumn,
    Entity as GlobalCountersEntity,
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
};

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

            active.updated_at = Set(timestamp);
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
                updated_at: Set(timestamp),
            };
            new.insert(db).await?;
        }
    }

    Ok(())
}
