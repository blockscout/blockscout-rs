use crate::types::counters::Counters;
use entity::counters_global_imported::{
    ActiveModel as GlobalCountersActiveModel, Column as GlobalCountersColumn,
    Entity as GlobalCountersEntity, Model as GlobalCountersModel,
};
use sea_orm::{
    ActiveValue::NotSet, ConnectionTrait, DbErr, EntityTrait, prelude::Expr, sea_query::OnConflict,
};

pub async fn upsert_many<C>(db: &C, counters: Vec<Counters>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let models: Vec<GlobalCountersModel> = counters
        .into_iter()
        .filter_map(|c| c.global)
        .map(Into::into)
        .collect();

    if models.is_empty() {
        return Ok(());
    }

    // assume the affected column set is the same for each entry in case of bulk import
    let affected_columns = get_affected_columns(&models[0]);

    let active_models: Vec<GlobalCountersActiveModel> = models
        .into_iter()
        .map(|m| {
            let mut active: GlobalCountersActiveModel = m.into();
            active.id = NotSet;
            active.created_at = NotSet;
            active.updated_at = NotSet;
            active
        })
        .collect();

    GlobalCountersEntity::insert_many(active_models)
        .on_conflict(
            OnConflict::columns([GlobalCountersColumn::ChainId, GlobalCountersColumn::Date])
                .update_columns(affected_columns)
                .value(GlobalCountersColumn::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}

fn get_affected_columns(model: &GlobalCountersModel) -> Vec<GlobalCountersColumn> {
    let mut columns = Vec::new();
    if model.daily_transactions_number.is_some() {
        columns.push(GlobalCountersColumn::DailyTransactionsNumber);
    }
    if model.total_transactions_number.is_some() {
        columns.push(GlobalCountersColumn::TotalTransactionsNumber);
    }
    if model.total_addresses_number.is_some() {
        columns.push(GlobalCountersColumn::TotalAddressesNumber);
    }

    columns
}
