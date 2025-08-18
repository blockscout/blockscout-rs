use crate::{chart_prelude::*, lines::NativeCoinHoldersGrowth};

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalNativeCoinHolders".into()
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

pub type TotalNativeCoinHolders =
    DirectPointLocalDbChartSource<LastPoint<StripExt<NativeCoinHoldersGrowth>>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_native_coin_holders() {
        simple_test_counter::<TotalNativeCoinHolders>(
            "update_total_native_coin_holders",
            "7",
            None,
        )
        .await;
    }
}
