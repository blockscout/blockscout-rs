use crate::{
    data_source::kinds::{
        data_manipulation::map::{Map, MapFunction, MapToString, UnwrapOr},
        local_db::DirectPointLocalDbChartSource,
    },
    gettable_const,
    types::TimespanValue,
    ChartError, ChartProperties, MissingDatePolicy, Named,
};
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::{Txns24hStats, TxnsStatsValue};

pub struct ExtractSum;

impl MapFunction<TimespanValue<NaiveDate, TxnsStatsValue>> for ExtractSum {
    type Output = TimespanValue<NaiveDate, Option<f64>>;

    fn function(
        inner_data: TimespanValue<NaiveDate, TxnsStatsValue>,
    ) -> Result<Self::Output, ChartError> {
        Ok(TimespanValue {
            timespan: inner_data.timespan,
            value: inner_data.value.fee_sum,
        })
    }
}

pub type TxnsFee24hExtracted = Map<Txns24hStats, ExtractSum>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "txnsFee24h".into()
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

gettable_const!(Zero: f64 = 0.0);

pub type TxnsFee24h =
    DirectPointLocalDbChartSource<MapToString<UnwrapOr<TxnsFee24hExtracted, Zero>>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_24h_1() {
        simple_test_counter::<TxnsFee24h>("update_txns_fee_24h_1", "0.000023592592569", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_24h_2() {
        simple_test_counter::<TxnsFee24h>(
            "update_txns_fee_24h_2",
            "0.000613407406794",
            Some(dt("2022-11-11T00:00:00")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee_24h_3() {
        simple_test_counter::<TxnsFee24h>(
            "update_txns_fee_24h_3",
            "0",
            Some(dt("2024-11-11T00:00:00")),
        )
        .await;
    }
}
