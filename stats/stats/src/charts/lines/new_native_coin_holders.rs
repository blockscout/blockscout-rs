use super::native_coin_holders_growth::NativeCoinHoldersGrowthInt;
use crate::{
    charts::{db_interaction::types::DateValueInt, Chart},
    data_source::kinds::updateable_chart::delta::{DeltaChart, DeltaChartWrapper},
    Named,
};
use entity::sea_orm_active_enums::ChartType;

pub struct NewNativeCoinHoldersInner;

impl Named for NewNativeCoinHoldersInner {
    const NAME: &'static str = "newNativeCoinHolders";
}

impl Chart for NewNativeCoinHoldersInner {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

impl DeltaChart for NewNativeCoinHoldersInner {
    type CumulativeChartPoint = DateValueInt;
    type CumulativeChart = NativeCoinHoldersGrowthInt;
}

pub type NewNativeCoinHolders = DeltaChartWrapper<NewNativeCoinHoldersInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_native_coin_holders() {
        simple_test_chart::<NewNativeCoinHolders>(
            "update_new_native_coin_holders",
            vec![
                ("2022-11-08", "0"),
                ("2022-11-09", "8"),
                ("2022-11-10", "0"),
                ("2022-11-11", "-1"),
            ],
        )
        .await;
    }
}
