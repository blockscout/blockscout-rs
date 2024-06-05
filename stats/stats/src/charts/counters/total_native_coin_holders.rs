use crate::{
    data_source::kinds::updateable_chart::last_point::{LastPointChart, LastPointChartWrapper},
    lines::NativeCoinHoldersGrowth,
    Chart, Named,
};
use entity::sea_orm_active_enums::ChartType;

pub struct TotalNativeCoinHoldersInner;

impl Named for TotalNativeCoinHoldersInner {
    const NAME: &'static str = "totalNativeCoinHolders";
}

impl Chart for TotalNativeCoinHoldersInner {
    fn chart_type() -> ChartType {
        ChartType::Counter
    }
}

impl LastPointChart for TotalNativeCoinHoldersInner {
    type InnerSource = NativeCoinHoldersGrowth;
}

pub type TotalNativeCoinHolders = LastPointChartWrapper<TotalNativeCoinHoldersInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_native_coin_holders() {
        simple_test_counter::<TotalNativeCoinHolders>("update_total_native_coin_holders", "7")
            .await;
    }
}
