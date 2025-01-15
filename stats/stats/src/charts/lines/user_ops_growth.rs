use crate::{
    charts::chart::ChartProperties,
    data_source::kinds::{
        data_manipulation::{map::StripExt, resolutions::last_value::LastValueLowerResolution},
        local_db::{
            parameters::update::batching::parameters::{Batch30Weeks, Batch30Years, Batch36Months},
            DailyCumulativeLocalDbChartSource, DirectVecLocalDbChartSource,
        },
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::new_user_ops::NewUserOpsInt;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "userOpsGrowth".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
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

pub type UserOpsGrowth = DailyCumulativeLocalDbChartSource<NewUserOpsInt, Properties>;
type UserOpsGrowthS = StripExt<UserOpsGrowth>;
pub type UserOpsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<UserOpsGrowthS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type UserOpsGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<UserOpsGrowthS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type UserOpsGrowthMonthlyS = StripExt<UserOpsGrowthMonthly>;
pub type UserOpsGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<UserOpsGrowthMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_user_ops_growth() {
        simple_test_chart::<UserOpsGrowth>(
            "update_user_ops_growth",
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "17"),
                ("2022-11-11", "31"),
                ("2022-11-12", "36"),
                ("2022-12-01", "41"),
                ("2023-01-01", "42"),
                ("2023-02-01", "46"),
                ("2023-03-01", "47"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_user_ops_growth_weekly() {
        simple_test_chart::<UserOpsGrowthWeekly>(
            "update_user_ops_growth_weekly",
            vec![
                ("2022-11-07", "36"),
                ("2022-11-28", "41"),
                ("2022-12-26", "42"),
                ("2023-01-30", "46"),
                ("2023-02-27", "47"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_user_ops_growth_monthly() {
        simple_test_chart::<UserOpsGrowthMonthly>(
            "update_user_ops_growth_monthly",
            vec![
                ("2022-11-01", "36"),
                ("2022-12-01", "41"),
                ("2023-01-01", "42"),
                ("2023-02-01", "46"),
                ("2023-03-01", "47"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_user_ops_growth_yearly() {
        simple_test_chart::<UserOpsGrowthYearly>(
            "update_user_ops_growth_yearly",
            vec![("2022-01-01", "41"), ("2023-01-01", "47")],
        )
        .await;
    }
}
