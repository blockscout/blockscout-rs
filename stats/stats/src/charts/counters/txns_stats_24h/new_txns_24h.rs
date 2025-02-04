use crate::{
    data_source::kinds::{
        data_manipulation::map::{Map, MapFunction, MapParseTo},
        local_db::DirectPointLocalDbChartSource,
    },
    types::TimespanValue,
    ChartError, ChartProperties, IndexingStatus, MissingDatePolicy, Named,
};
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::{Txns24hStats, TxnsStatsValue};

pub struct ExtractCount;

impl MapFunction<TimespanValue<NaiveDate, TxnsStatsValue>> for ExtractCount {
    type Output = TimespanValue<NaiveDate, String>;

    fn function(
        inner_data: TimespanValue<NaiveDate, TxnsStatsValue>,
    ) -> Result<Self::Output, ChartError> {
        Ok(TimespanValue {
            timespan: inner_data.timespan,
            value: inner_data.value.count.to_string(),
        })
    }
}

pub type NewTxns24hExtracted = Map<Txns24hStats, ExtractCount>;
pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTxns24h".into()
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
        IndexingStatus::NoneIndexed
    }
}

pub type NewTxns24h = DirectPointLocalDbChartSource<NewTxns24hExtracted, Properties>;

pub type NewTxns24hInt = MapParseTo<NewTxns24h, i64>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_24h_1() {
        simple_test_counter::<NewTxns24h>("update_new_txns_24h_1", "1", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_24h_2() {
        simple_test_counter::<NewTxns24h>(
            "update_new_txns_24h_2",
            // block at `2022-11-11T00:00:00` is not counted because
            // the relation is 'less than' in query
            "12",
            Some(dt("2022-11-11T00:00:00")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_24h_3() {
        simple_test_counter::<NewTxns24h>(
            "update_new_txns_24h_3",
            "0",
            Some(dt("2024-11-11T00:00:00")),
        )
        .await;
    }
}
