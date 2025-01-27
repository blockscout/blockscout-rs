use std::marker::PhantomData;

use crate::{
    data_source::kinds::{
        data_manipulation::map::{Map, MapFunction},
        local_db::DirectPointLocalDbChartSource,
    },
    types::TimespanValue,
    ChartProperties, IndexingStatus, MissingDatePolicy, Named,
};
use std::fmt::Debug;

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use tracing::warn;

use super::{TotalBlocksInt, TotalTxnsInt};

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalOperationalTxns".into()
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

pub struct CalculateOperationalTxns<ChartName: Named>(PhantomData<ChartName>);

type Input<Resolution> = (
    // blocks
    TimespanValue<Resolution, i64>,
    // all transactions
    TimespanValue<Resolution, i64>,
);

impl<Resolution, ChartName> MapFunction<Input<Resolution>> for CalculateOperationalTxns<ChartName>
where
    Resolution: Debug + PartialEq + Send,
    ChartName: Named,
{
    type Output = TimespanValue<Resolution, String>;

    fn function(inner_data: Input<Resolution>) -> Result<Self::Output, crate::ChartError> {
        let (total_blocks_data, total_txns_data) = inner_data;
        if total_blocks_data.timespan != total_txns_data.timespan {
            warn!("timespans for total blocks and total transactions do not match when calculating {}: \
            {:?} != {:?}", ChartName::name(), total_blocks_data.timespan, total_txns_data.timespan);
        }
        let date = total_blocks_data.timespan;
        let value = total_txns_data
            .value
            .saturating_sub(total_blocks_data.value);
        Ok(TimespanValue {
            timespan: date,
            value: value.to_string(),
        })
    }
}

pub type TotalOperationalTxns = DirectPointLocalDbChartSource<
    Map<(TotalBlocksInt, TotalTxnsInt), CalculateOperationalTxns<Properties>>,
    Properties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_operational_txns() {
        // 48 - 13 (txns - blocks)
        simple_test_counter::<TotalOperationalTxns>("update_total_operational_txns", "44", None)
            .await;
    }
}
