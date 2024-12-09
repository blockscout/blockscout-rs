use crate::{
    charts::db_interaction::read::query_estimated_table_rows,
    data_source::kinds::{
        data_manipulation::{map::MapToString, sum_point::Sum},
        local_db::{parameters::ValueEstimation, DirectPointLocalDbChartSourceWithEstimate},
    },
    lines::NewTxnsInt,
    types::timespans::DateValue,
    utils::MarkedDbConnection,
    ChartError, ChartProperties, MissingDatePolicy, Named,
};

use blockscout_db::entity::transactions;
use chrono::{NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::EntityName;

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

pub type TotalTxns = DirectPointLocalDbChartSourceWithEstimate<
    MapToString<Sum<NewTxnsInt>>,
    TotalTxnsEstimation,
    Properties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::{simple_test_counter, test_counter_fallback};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_txns() {
        simple_test_counter::<TotalTxns>("update_total_txns", "47", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_txns_fallback() {
        test_counter_fallback::<TotalTxns>("total_txns_fallback").await;
    }
}
