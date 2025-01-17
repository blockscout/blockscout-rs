use crate::{
    data_source::kinds::{
        data_manipulation::map::{Map, MapFunction, MapToString, UnwrapOr},
        local_db::DirectPointLocalDbChartSource,
    },
    gettable_const,
    types::TimespanValue,
    ChartProperties, MissingDatePolicy, Named,
};
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::{Txns24hStats, TxnsStatsValue};

pub struct ExtractAverage;

impl MapFunction<TimespanValue<NaiveDate, TxnsStatsValue>> for ExtractAverage {
    type Output = TimespanValue<NaiveDate, Option<f64>>;

    fn function(
        inner_data: TimespanValue<NaiveDate, TxnsStatsValue>,
    ) -> Result<Self::Output, crate::ChartError> {
        Ok(TimespanValue {
            timespan: inner_data.timespan,
            value: inner_data.value.fee_average,
        })
    }
}

pub type AverageTxnFee24hExtracted = Map<Txns24hStats, ExtractAverage>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "averageTxnFee24h".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillZero
    }
}

gettable_const!(Zero: f64 = 0.0);

pub type AverageTxnFee24h = DirectPointLocalDbChartSource<
    MapToString<UnwrapOr<AverageTxnFee24hExtracted, Zero>>,
    Properties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txns_fee_1() {
        simple_test_counter::<AverageTxnFee24h>(
            "update_average_txns_fee_1",
            "0.000023592592569",
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txns_fee_2() {
        simple_test_counter::<AverageTxnFee24h>(
            "update_average_txns_fee_2",
            "0.0000754962962208",
            Some(dt("2022-11-11T16:00:00")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_txns_fee_3() {
        simple_test_counter::<AverageTxnFee24h>(
            "update_average_txns_fee_3",
            "0",
            Some(dt("2024-10-10T00:00:00")),
        )
        .await;
    }
}
