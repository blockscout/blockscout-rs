use crate::{
    data_source::kinds::{data_manipulation::map::Map, local_db::DirectPointLocalDbChartSource},
    indexing_status::{BlockscoutIndexingStatus, IndexingStatusTrait, UserOpsIndexingStatus},
    types::new_txns::ExtractOpStackTxns,
    ChartProperties, IndexingStatus, MissingDatePolicy, Named,
};
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::YesterdayTxnsCombinedRemote;

pub type OpStackYesterdayOperationalTxnsRemote =
    Map<YesterdayTxnsCombinedRemote, ExtractOpStackTxns>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "opStackYesterdayOperationalTxns".into()
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

pub type OpStackYesterdayOperationalTxns =
    DirectPointLocalDbChartSource<OpStackYesterdayOperationalTxnsRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_op_stack_yesterday_operational_txns() {
        simple_test_counter::<OpStackYesterdayOperationalTxns>(
            "update_op_stack_yesterday_operational_txns",
            "1",
            Some(dt("2023-03-02T00:00:00")),
        )
        .await;
    }
}
