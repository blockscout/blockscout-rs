use crate::{
    data_source::kinds::{
        data_manipulation::{
            delta::Delta,
            map::{MapDivide, MapParseTo, MapToString, StripExt},
            resolutions::average::AverageLowerResolution,
        },
        local_db::{
            parameters::update::batching::parameters::{
                Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
            },
            DirectVecLocalDbChartSource,
        },
    },
    define_and_impl_resolution_properties,
    lines::blocks::new_blocks::NewBlocksInt,
    types::timespans::{Month, Week, Year},
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::gas_used_growth::GasUsedGrowthInt;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "averageGasUsed".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

define_and_impl_resolution_properties!(
    define_and_impl: {
        WeeklyProperties: Week,
        MonthlyProperties: Month,
        YearlyProperties: Year,
    },
    base_impl: Properties
);

pub type TotalGasUsed = Delta<GasUsedGrowthInt>;

pub type AverageGasUsed = DirectVecLocalDbChartSource<
    MapToString<MapDivide<TotalGasUsed, NewBlocksInt>>,
    Batch30Days,
    Properties,
>;
pub type AverageGasUsedFloat = MapParseTo<StripExt<AverageGasUsed>, f64>;

pub type AverageGasUsedWeekly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<AverageGasUsedFloat, NewBlocksInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AverageGasUsedWeeklyFloat = MapParseTo<StripExt<AverageGasUsedWeekly>, f64>;

pub type AverageGasUsedMonthly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<AverageGasUsedFloat, NewBlocksInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type AverageGasUsedMonthlyFloat = MapParseTo<StripExt<AverageGasUsedMonthly>, f64>;

pub type AverageGasUsedYearly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<AverageGasUsedFloat, NewBlocksInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;
pub type AverageGasUsedYearlyFloat = MapParseTo<StripExt<AverageGasUsedYearly>, f64>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_used() {
        simple_test_chart::<AverageGasUsed>(
            "update_average_gas_used",
            vec![
                ("2022-11-09", "10000"),
                ("2022-11-10", "27260"),
                ("2022-11-11", "32465"),
                ("2022-11-12", "29040"),
                ("2022-12-01", "37670"),
                ("2023-01-01", "46300"),
                ("2023-02-01", "54930"),
                ("2023-03-01", "13560"),
            ],
        )
        .await;
    }
}
