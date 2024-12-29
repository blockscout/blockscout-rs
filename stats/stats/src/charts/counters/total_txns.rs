use crate::{
    charts::db_interaction::read::query_estimated_table_rows,
    data_source::{
        kinds::{
            data_manipulation::map::MapParseTo,
            local_db::{parameters::ValueEstimation, DirectPointLocalDbChartSourceWithEstimate},
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        UpdateContext,
    },
    range::UniversalRange,
    types::timespans::DateValue,
    ChartError, ChartProperties, MissingDatePolicy, Named,
};

use blockscout_db::entity::{blocks, transactions};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{
    prelude::Expr, ColumnTrait, DatabaseConnection, EntityName, EntityTrait, PaginatorTrait,
    QueryFilter, QuerySelect,
};

pub struct TotalTxnsQueryBehaviour;

impl RemoteQueryBehaviour for TotalTxnsQueryBehaviour {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let blockscout = cx.blockscout;
        let timespan: NaiveDateTime = blocks::Entity::find()
            .select_only()
            .column_as(Expr::col(blocks::Column::Timestamp).max(), "timestamp")
            .filter(blocks::Column::Consensus.eq(true))
            .into_tuple()
            .one(blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?
            .ok_or_else(|| ChartError::Internal("no block timestamps in database".into()))?;

        let value = transactions::Entity::find()
            .select_only()
            .count(blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?;

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
}

pub struct TotalTxnsEstimation;

impl ValueEstimation for TotalTxnsEstimation {
    async fn estimate(blockscout: &DatabaseConnection) -> Result<DateValue<String>, ChartError> {
        // `now()` is more relevant when taken right before the query rather than
        // `cx.time` measured a bit earlier.
        let now = Utc::now();
        let value = query_estimated_table_rows(blockscout, transactions::Entity.table_name())
            .await
            .map_err(ChartError::BlockscoutDB)?
            .map(|n| u64::try_from(n).unwrap_or(0))
            .unwrap_or(0);
        Ok(DateValue {
            timespan: now.date_naive(),
            value: value.to_string(),
        })
    }
}

// We will need it to update on not fully indexed data soon, therefore this counter is
// separated from `NewTxns`.
//
// Separate query not reliant on previous computation helps this counter to work in such
// environments.
//
// todo: make it dependent again if #845 is resolved
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
        simple_test_counter::<TotalTxns>("update_total_txns", "48", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_txns_fallback() {
        test_counter_fallback::<TotalTxns>("total_txns_fallback").await;
    }
}
