use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        UpdateContext,
    },
    range::UniversalRange,
    types::TimespanValue,
    ChartError, ChartProperties, MissingDatePolicy, Named,
};

use blockscout_db::entity::transactions;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{
    ColumnTrait, DbBackend, EntityTrait, FromQueryResult, QueryFilter, QuerySelect, QueryTrait,
    Statement,
};

pub struct PendingTxnsStatement;

impl PendingTxnsStatement {
    fn get_statement(inserted_from: DateTime<Utc>) -> Statement {
        transactions::Entity::find()
            .select_only()
            .filter(transactions::Column::BlockHash.is_null())
            .filter(transactions::Column::InsertedAt.gte(inserted_from))
            .column_as(transactions::Column::Hash.count(), "value")
            .build(DbBackend::Postgres)
    }
}

#[derive(FromQueryResult)]
struct Value {
    value: i64,
}

pub struct PendingTxnsQuery;

impl RemoteQueryBehaviour for PendingTxnsQuery {
    type Output = TimespanValue<NaiveDate, String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let update_time = cx.time;
        let query = PendingTxnsStatement::get_statement(
            update_time
                .checked_sub_signed(TimeDelta::minutes(30))
                .unwrap_or(DateTime::<Utc>::MIN_UTC),
        );
        let data = Value::find_by_statement(query)
            .one(cx.blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?
            .ok_or_else(|| ChartError::Internal("query returned nothing".into()))?;
        Ok(TimespanValue {
            timespan: update_time.date_naive(),
            value: data.value.to_string(),
        })
    }
}

pub type PendingTxnsRemote = RemoteDatabaseSource<PendingTxnsQuery>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "pendingTxns".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type PendingTxns = DirectPointLocalDbChartSource<PendingTxnsRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter_with_migration_variants;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_pending_txns() {
        simple_test_counter_with_migration_variants::<PendingTxns>(
            "update_pending_txns",
            "0",
            None,
        )
        .await;
    }
}
