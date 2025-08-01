use crate::{
    ChartError, ChartProperties, IndexingStatus, MissingDatePolicy, Named,
    charts::db_interaction::read::find_one_value,
    data_source::{
        UpdateContext,
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
    },
    indexing_status::{BlockscoutIndexingStatus, IndexingStatusTrait, UserOpsIndexingStatus},
    range::UniversalRange,
    types::TimespanValue,
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

pub struct PendingTxns30mQuery;

impl RemoteQueryBehaviour for PendingTxns30mQuery {
    type Output = TimespanValue<NaiveDate, String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let update_time = cx.time;
        let statement = PendingTxnsStatement::get_statement(
            update_time
                .checked_sub_signed(TimeDelta::minutes(30))
                .unwrap_or(DateTime::<Utc>::MIN_UTC),
        );
        let data = find_one_value::<Value>(cx, statement)
            .await?
            .ok_or_else(|| ChartError::Internal("query returned nothing".into()))?;
        Ok(TimespanValue {
            timespan: update_time.date_naive(),
            value: data.value.to_string(),
        })
    }
}

pub type PendingTxns30mRemote = RemoteDatabaseSource<PendingTxns30mQuery>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "pendingTxns30m".into()
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
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus {
            blockscout: BlockscoutIndexingStatus::NoneIndexed,
            user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
        }
    }
}

pub type PendingTxns30m = DirectPointLocalDbChartSource<PendingTxns30mRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_pending_txns_30m() {
        simple_test_counter::<PendingTxns30m>("update_pending_txns_30m", "0", None).await;
    }
}
