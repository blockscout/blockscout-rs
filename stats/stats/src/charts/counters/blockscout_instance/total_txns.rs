use crate::{
    ChartError, ChartProperties, IndexingStatus, MissingDatePolicy, Named,
    charts::db_interaction::read::query_estimated_table_rows,
    data_source::{
        UpdateContext,
        kinds::{
            data_manipulation::map::MapParseTo,
            local_db::{DirectPointLocalDbChartSourceWithEstimate, parameters::ValueEstimation},
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
    },
    indexing_status::{BlockscoutIndexingStatus, IndexingStatusTrait, UserOpsIndexingStatus},
    range::UniversalRange,
    types::timespans::DateValue,
};

use blockscout_db::entity::{blocks, transactions};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityName, EntityTrait, PaginatorTrait, QueryFilter,
    QuerySelect, prelude::Expr,
};

pub struct TotalTxnsQueryBehaviour;

impl RemoteQueryBehaviour for TotalTxnsQueryBehaviour {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let blockscout = cx.indexer_db;
        let timespan: NaiveDateTime = blocks::Entity::find()
            .select_only()
            .column_as(Expr::col(blocks::Column::Timestamp).max(), "timestamp")
            .filter(blocks::Column::Consensus.eq(true))
            .into_tuple()
            .one(blockscout)
            .await
            .map_err(ChartError::IndexerDB)?
            .ok_or_else(|| ChartError::Internal("no block timestamps in database".into()))?;

        let value = transactions::Entity::find()
            .select_only()
            .count(blockscout)
            .await
            .map_err(ChartError::IndexerDB)?;

        let data = DateValue::<String> {
            timespan: timespan.date(),
            value: value.to_string(),
        };
        Ok(data)
    }
}

pub type TotalTxnsRemote = RemoteDatabaseSource<TotalTxnsQueryBehaviour>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalTxns".into()
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

pub struct TotalTxnsEstimation;

impl ValueEstimation for TotalTxnsEstimation {
    async fn estimate(blockscout: &DatabaseConnection) -> Result<DateValue<String>, ChartError> {
        // `now()` is more relevant when taken right before the query rather than
        // `cx.time` measured a bit earlier.
        let now = Utc::now();
        let value = query_estimated_table_rows(blockscout, transactions::Entity.table_name())
            .await
            .map_err(ChartError::IndexerDB)?
            .map(|n| u64::try_from(n).unwrap_or(0))
            .unwrap_or(0);
        Ok(DateValue {
            timespan: now.date_naive(),
            value: value.to_string(),
        })
    }
}

// Independent from `NewTxns` because this needs to work on not-fully-indexed
// just as well.
pub type TotalTxns =
    DirectPointLocalDbChartSourceWithEstimate<TotalTxnsRemote, TotalTxnsEstimation, Properties>;
pub type TotalTxnsInt = MapParseTo<TotalTxns, i64>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::{simple_test_counter, test_counter_fallback};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_txns() {
        simple_test_counter::<TotalTxns>("update_total_txns", "58", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_txns_fallback() {
        test_counter_fallback::<TotalTxns>("total_txns_fallback").await;
    }
}
