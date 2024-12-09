use crate::{
    charts::db_interaction::read::query_estimated_table_rows,
    data_source::{
        kinds::{
            local_db::{parameters::ValueEstimation, DirectPointLocalDbChartSourceWithEstimate},
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        UpdateContext,
    },
    range::UniversalRange,
    types::timespans::DateValue,
    utils::MarkedDbConnection,
    ChartError, ChartProperties, MissingDatePolicy, Named,
};

use blockscout_db::entity::transactions;
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{EntityName, EntityTrait, PaginatorTrait, QuerySelect};

pub struct TotalTxnsQueryBehaviour;

impl RemoteQueryBehaviour for TotalTxnsQueryBehaviour {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let now = cx.time;
        let value = transactions::Entity::find()
            .select_only()
            .count(cx.blockscout.connection.as_ref())
            .await
            .map_err(ChartError::BlockscoutDB)?;

        let data = DateValue::<String> {
            timespan: now.date_naive(),
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
    async fn estimate(blockscout: &MarkedDbConnection) -> Result<DateValue<String>, ChartError> {
        let now = Utc::now();
        let value = query_estimated_table_rows(
            blockscout.connection.as_ref(),
            transactions::Entity.table_name(),
        )
        .await
        .map_err(ChartError::BlockscoutDB)?
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
// todo: make it dependant again if #845 is resolved
pub type TotalTxns =
    DirectPointLocalDbChartSourceWithEstimate<TotalTxnsRemote, TotalTxnsEstimation, Properties>;

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
