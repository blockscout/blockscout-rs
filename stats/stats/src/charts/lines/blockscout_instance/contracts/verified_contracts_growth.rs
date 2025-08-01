use crate::{
    MissingDatePolicy, Named,
    charts::chart::ChartProperties,
    data_source::kinds::{
        data_manipulation::{map::StripExt, resolutions::last_value::LastValueLowerResolution},
        local_db::{
            DailyCumulativeLocalDbChartSource, DirectVecLocalDbChartSource,
            parameters::update::batching::parameters::{Batch30Weeks, Batch30Years, Batch36Months},
        },
    },
    define_and_impl_resolution_properties,
    lines::NewVerifiedContractsInt,
    types::timespans::{Month, Week, Year},
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "verifiedContractsGrowth".into()
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

pub type VerifiedContractsGrowth =
    DailyCumulativeLocalDbChartSource<NewVerifiedContractsInt, Properties>;
type VerifiedContractsGrowthS = StripExt<VerifiedContractsGrowth>;
pub type VerifiedContractsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<VerifiedContractsGrowthS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type VerifiedContractsGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<VerifiedContractsGrowthS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type VerifiedContractsGrowthMonthlyS = StripExt<VerifiedContractsGrowthMonthly>;
pub type VerifiedContractsGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<VerifiedContractsGrowthMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_verified_contracts_growth() {
        simple_test_chart::<VerifiedContractsGrowth>(
            "update_verified_contracts_growth",
            vec![
                ("2022-11-14", "1"),
                ("2022-11-15", "2"),
                ("2022-11-16", "3"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_verified_contracts_growth_weekly() {
        simple_test_chart::<VerifiedContractsGrowthWeekly>(
            "update_verified_contracts_growth_weekly",
            vec![("2022-11-14", "3")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_verified_contracts_growth_monthly() {
        simple_test_chart::<VerifiedContractsGrowthMonthly>(
            "update_verified_contracts_growth_monthly",
            vec![("2022-11-01", "3")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_verified_contracts_growth_yearly() {
        simple_test_chart::<VerifiedContractsGrowthYearly>(
            "update_verified_contracts_growth_yearly",
            vec![("2022-01-01", "3")],
        )
        .await;
    }
}
