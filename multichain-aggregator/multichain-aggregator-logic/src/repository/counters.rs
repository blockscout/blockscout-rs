use crate::types::counters::ChainCounters;
use entity::counters_global_imported::{
    ActiveModel as GlobalCountersActiveModel, Column as GlobalCountersColumn,
    Entity as GlobalCountersEntity, Model as GlobalCountersModel,
};
use sea_orm::{
    prelude::Expr, sea_query::OnConflict, ActiveValue::NotSet, ConnectionTrait, DbErr, EntityTrait,
};

pub async fn upsert_chain_counters<C>(db: &C, data: ChainCounters) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let model: GlobalCountersModel = data.into();
    let affected_columns = get_affected_columns(&model);

    let mut active_model: GlobalCountersActiveModel = model.into();
    active_model.id = NotSet;
    active_model.created_at = NotSet;
    active_model.updated_at = NotSet;

    GlobalCountersEntity::insert(active_model)
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
