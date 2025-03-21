use crate::{
    data_source::kinds::{
        data_manipulation::map::{Map, MapFunction},
        local_db::DirectPointLocalDbChartSource,
    },
    indexing_status::{BlockscoutIndexingStatus, IndexingStatusTrait, UserOpsIndexingStatus},
    types::TimespanValue,
    ChartError, ChartProperties, IndexingStatus, MissingDatePolicy, Named,
};
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::{Txns24hStats, TxnsStatsValue};

pub struct ExtractOpStackOperationalCount;

impl MapFunction<TimespanValue<NaiveDate, TxnsStatsValue>> for ExtractOpStackOperationalCount {
    type Output = TimespanValue<NaiveDate, String>;

    fn function(
        inner_data: TimespanValue<NaiveDate, TxnsStatsValue>,
    ) -> Result<Self::Output, ChartError> {
        Ok(TimespanValue {
            timespan: inner_data.timespan,
            value: inner_data.value.count_op_stack_operational.to_string(),
        })
    }
}

pub type OpStackNewOperationalTxns24hExtracted = Map<Txns24hStats, ExtractOpStackOperationalCount>;
pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "opStackNewOperationalTxns24h".into()
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

pub type OpStackNewOperationalTxns24h =
    DirectPointLocalDbChartSource<OpStackNewOperationalTxns24hExtracted, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_op_stack_new_operational_txns_24h() {
        simple_test_counter::<OpStackNewOperationalTxns24h>(
            "update_op_stack_new_operational_txns_24h",
            "1",
            None,
        )
        .await;
    }
}
