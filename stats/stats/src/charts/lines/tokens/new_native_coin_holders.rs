use crate::{
    charts::ChartProperties,
    data_source::kinds::{
        data_manipulation::{
            delta::Delta,
            map::{MapParseTo, MapToString, StripExt},
            resolutions::sum::SumLowerResolution,
        },
        local_db::{
            parameters::update::batching::parameters::{
                Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
            },
            DirectVecLocalDbChartSource,
        },
    },
    define_and_impl_resolution_properties,
    lines::native_coin_holders_growth::NativeCoinHoldersGrowthInt,
    types::timespans::{Month, Week, Year},
    Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newNativeCoinHolders".into()
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

pub type NewNativeCoinHolders = DirectVecLocalDbChartSource<
    MapToString<Delta<NativeCoinHoldersGrowthInt>>,
    Batch30Days,
    Properties,
>;
pub type NewNativeCoinHoldersInt = MapParseTo<StripExt<NewNativeCoinHolders>, i64>;
pub type NewNativeCoinHoldersWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewNativeCoinHoldersInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewNativeCoinHoldersMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewNativeCoinHoldersInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewNativeCoinHoldersMonthlyInt = MapParseTo<StripExt<NewNativeCoinHoldersMonthly>, i64>;
pub type NewNativeCoinHoldersYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewNativeCoinHoldersMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
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

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_native_coin_holders_weekly() {
        simple_test_chart::<NewNativeCoinHoldersWeekly>(
            "update_new_native_coin_holders_weekly",
            vec![("2022-11-07", "7")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_native_coin_holders_monthly() {
        simple_test_chart::<NewNativeCoinHoldersMonthly>(
            "update_new_native_coin_holders_monthly",
            vec![("2022-11-01", "7")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_native_coin_holders_yearly() {
        simple_test_chart::<NewNativeCoinHoldersYearly>(
            "update_new_native_coin_holders_yearly",
            vec![("2022-01-01", "7")],
        )
        .await;
    }
}
