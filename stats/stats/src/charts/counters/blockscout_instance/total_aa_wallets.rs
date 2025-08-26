use crate::{chart_prelude::*, lines::AccountAbstractionWalletsGrowth};

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalAccountAbstractionWallets".into()
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
            .with_blockscout(BlockscoutIndexingStatus::BlocksIndexed)
            .with_user_ops(UserOpsIndexingStatus::PastOperationsIndexed)
    }
}

pub type TotalAccountAbstractionWallets =
    DirectPointLocalDbChartSource<LastPoint<StripExt<AccountAbstractionWalletsGrowth>>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_account_abstraction_wallets() {
        simple_test_counter::<TotalAccountAbstractionWallets>(
            "update_total_account_abstraction_wallets",
            "1",
            None,
        )
        .await;
    }
}
