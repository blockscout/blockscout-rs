use crate::{
    charts::ChartProperties,
    data_source::kinds::{
        data_manipulation::{delta::Delta, map::MapToString},
        local_db::{
            parameters::update::batching::parameters::Batch30Days, DirectVecLocalDbChartSource,
        },
    },
    lines::native_coin_holders_growth::NativeCoinHoldersGrowthInt,
    Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct NewNativeCoinHoldersProperties;

impl Named for NewNativeCoinHoldersProperties {
    fn name() -> String {
        "newNativeCoinHolders".into()
    }
}

impl ChartProperties for NewNativeCoinHoldersProperties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type NewNativeCoinHolders = DirectVecLocalDbChartSource<
    MapToString<Delta<NativeCoinHoldersGrowthInt>>,
    Batch30Days,
    NewNativeCoinHoldersProperties,
>;

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
                ("2022-11-09", "8"),
                ("2022-11-10", "0"),
                ("2022-11-11", "-1"),
            ],
        )
        .await;
    }
}
