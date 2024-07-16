use crate::{
    data_source::kinds::{
        data_manipulation::last_point::LastPoint, local_db::DirectPointLocalDbChartSource,
    },
    lines::NativeCoinHoldersGrowth,
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct TotalNativeCoinHoldersProperties;

impl Named for TotalNativeCoinHoldersProperties {
    const NAME: &'static str = "totalNativeCoinHolders";
}

impl ChartProperties for TotalNativeCoinHoldersProperties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type TotalNativeCoinHolders = DirectPointLocalDbChartSource<
    LastPoint<NativeCoinHoldersGrowth>,
    TotalNativeCoinHoldersProperties,
>;

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
