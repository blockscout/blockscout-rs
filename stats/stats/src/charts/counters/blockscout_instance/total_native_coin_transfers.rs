use crate::{chart_prelude::*, lines::NewNativeCoinTransfersInt};

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalNativeCoinTransfers".into()
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

pub type TotalNativeCoinTransfers =
    DirectPointLocalDbChartSource<MapToString<Sum<NewNativeCoinTransfersInt>>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_native_coin_transfers() {
        simple_test_counter::<TotalNativeCoinTransfers>(
            "update_total_native_coin_transfers",
            "17",
            None,
        )
        .await;
    }
}
