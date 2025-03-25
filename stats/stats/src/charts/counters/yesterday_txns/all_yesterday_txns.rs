use crate::{
    data_source::kinds::{
        data_manipulation::map::{Map, MapParseTo},
        local_db::DirectPointLocalDbChartSource,
    },
    indexing_status::{BlockscoutIndexingStatus, IndexingStatusTrait, UserOpsIndexingStatus},
    types::new_txns::ExtractAllTxns,
    ChartProperties, IndexingStatus, MissingDatePolicy, Named,
};
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::YesterdayTxnsCombinedRemote;

pub type YesterdayTxnsRemote = Map<YesterdayTxnsCombinedRemote, ExtractAllTxns>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "yesterdayTxns".into()
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

pub type YesterdayTxns = DirectPointLocalDbChartSource<YesterdayTxnsRemote, Properties>;
pub type YesterdayTxnsInt = MapParseTo<YesterdayTxns, i64>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_yesterday_txns_1() {
        simple_test_counter::<YesterdayTxns>("update_yesterday_txns_1", "0", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_yesterday_txns_2() {
        simple_test_counter::<YesterdayTxns>(
            "update_yesterday_txns_2",
            "14",
            Some(dt("2022-11-11T00:00:00")),
        )
        .await;
    }
}
