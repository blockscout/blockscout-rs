use crate::{
    ChartProperties, IndexingStatus, MissingDatePolicy, Named,
    data_source::kinds::{
        data_manipulation::{last_point::LastPoint, map::StripExt},
        local_db::DirectPointLocalDbChartSource,
    },
    indexing_status::{BlockscoutIndexingStatus, IndexingStatusTrait},
    lines::BuilderAccountsGrowth,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalBuilderAccounts".into()
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
        IndexingStatus::LEAST_RESTRICTIVE
            .with_blockscout(BlockscoutIndexingStatus::InternalTransactionsIndexed)
    }
}

pub type TotalBuilderAccounts =
    DirectPointLocalDbChartSource<LastPoint<StripExt<BuilderAccountsGrowth>>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_builder_accounts() {
        simple_test_counter::<TotalBuilderAccounts>("update_total_builder_accounts", "8", None)
            .await;
    }
}
