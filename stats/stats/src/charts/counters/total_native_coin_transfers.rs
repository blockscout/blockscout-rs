use crate::{
    data_source::kinds::{
        data_manipulation::{map::MapToString, sum_point::Sum},
        local_db::DirectPointLocalDbChartSource,
    },
    lines::NewNativeCoinTransfersInt,
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct TotalNativeCoinTransfersProperties;

impl Named for TotalNativeCoinTransfersProperties {
    const NAME: &'static str = "totalNativeCoinTransfers";
}

impl ChartProperties for TotalNativeCoinTransfersProperties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type TotalNativeCoinTransfers = DirectPointLocalDbChartSource<
    MapToString<Sum<NewNativeCoinTransfersInt>>,
    TotalNativeCoinTransfersProperties,
>;

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
