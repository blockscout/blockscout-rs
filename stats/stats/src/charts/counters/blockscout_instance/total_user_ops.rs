use crate::{
    ChartProperties, IndexingStatus, MissingDatePolicy, Named,
    data_source::kinds::{
        data_manipulation::{last_point::LastPoint, map::StripExt},
        local_db::DirectPointLocalDbChartSource,
    },
    indexing_status::{BlockscoutIndexingStatus, UserOpsIndexingStatus},
    lines::UserOpsGrowth,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalUserOps".into()
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
            blockscout: BlockscoutIndexingStatus::BlocksIndexed,
            user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
        }
    }
}

pub type TotalUserOps =
    DirectPointLocalDbChartSource<LastPoint<StripExt<UserOpsGrowth>>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_user_ops() {
        simple_test_counter::<TotalUserOps>("update_total_user_ops", "8", None).await;
    }
}
