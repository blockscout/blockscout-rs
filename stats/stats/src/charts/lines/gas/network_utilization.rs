use crate::{
    data_source::kinds::{
        data_manipulation::map::{MapDivide, MapToString},
        local_db::{
            parameters::update::batching::parameters::{
                Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
            },
            DirectVecLocalDbChartSource,
        },
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::{
    average_gas_limit::{
        AverageGasLimitFloat, AverageGasLimitMonthlyFloat, AverageGasLimitWeeklyFloat,
        AverageGasLimitYearlyFloat,
    },
    average_gas_used::{
        AverageGasUsedFloat, AverageGasUsedMonthlyFloat, AverageGasUsedWeeklyFloat,
        AverageGasUsedYearlyFloat,
    },
};

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "networkUtilization".into()
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

pub type NetworkUtilization = DirectVecLocalDbChartSource<
    MapToString<MapDivide<AverageGasUsedFloat, AverageGasLimitFloat>>,
    Batch30Days,
    Properties,
>;
// correct weight for AverageLowerResolution would be "total gas limit",
// but it's quite inconvenient to compute
pub type NetworkUtilizationWeekly = DirectVecLocalDbChartSource<
    MapToString<MapDivide<AverageGasUsedWeeklyFloat, AverageGasLimitWeeklyFloat>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NetworkUtilizationMonthly = DirectVecLocalDbChartSource<
    MapToString<MapDivide<AverageGasUsedMonthlyFloat, AverageGasLimitMonthlyFloat>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NetworkUtilizationYearly = DirectVecLocalDbChartSource<
    MapToString<MapDivide<AverageGasUsedYearlyFloat, AverageGasLimitYearlyFloat>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_network_utilization() {
        simple_test_chart::<NetworkUtilization>(
            "update_network_utilization",
            vec![
                ("2022-11-09", "0.0008"),
                ("2022-11-10", "0.0021808"),
                ("2022-11-11", "0.0010821666666666666"),
                ("2022-11-12", "0.000968"),
                ("2022-12-01", "0.0012556666666666666"),
                ("2023-01-01", "0.0015433333333333334"),
                ("2023-02-01", "0.001831"),
                ("2023-03-01", "0.000452"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_network_utilization_weekly() {
        simple_test_chart::<NetworkUtilizationWeekly>(
            "update_network_utilization_weekly",
            vec![
                ("2022-11-07", "0.0012533999999999998"),
                ("2022-11-28", "0.0012556666666666666"),
                ("2022-12-26", "0.0015433333333333334"),
                ("2023-01-30", "0.001831"),
                ("2023-02-27", "0.000452"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_network_utilization_monthly() {
        simple_test_chart::<NetworkUtilizationMonthly>(
            "update_network_utilization_monthly",
            vec![
                ("2022-11-01", "0.0012533999999999998"),
                ("2022-12-01", "0.0012556666666666666"),
                ("2023-01-01", "0.0015433333333333334"),
                ("2023-02-01", "0.001831"),
                ("2023-03-01", "0.000452"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_network_utilization_yearly() {
        simple_test_chart::<NetworkUtilizationYearly>(
            "update_network_utilization_yearly",
            vec![
                ("2022-01-01", "0.001253695652173913"),
                ("2023-01-01", "0.0012754444444444445"),
            ],
        )
        .await;
    }
}
