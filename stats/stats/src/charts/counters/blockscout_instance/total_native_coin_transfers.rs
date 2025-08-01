use crate::{
    ChartProperties, MissingDatePolicy, Named,
    data_source::kinds::{
        data_manipulation::{map::MapToString, sum_point::Sum},
        local_db::DirectPointLocalDbChartSource,
    },
    lines::NewNativeCoinTransfersInt,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

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
