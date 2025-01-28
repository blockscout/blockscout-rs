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

use super::new_aa_wallets::NewAccountAbstractionWalletsInt;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "accountAbstractionWalletsGrowth".into()
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

pub type AccountAbstractionWalletsGrowth =
    DailyCumulativeLocalDbChartSource<NewAccountAbstractionWalletsInt, Properties>;
type AccountAbstractionWalletsGrowthS = StripExt<AccountAbstractionWalletsGrowth>;
pub type AccountAbstractionWalletsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<AccountAbstractionWalletsGrowthS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AccountAbstractionWalletsGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<AccountAbstractionWalletsGrowthS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type AccountAbstractionWalletsGrowthMonthlyS = StripExt<AccountAbstractionWalletsGrowthMonthly>;
pub type AccountAbstractionWalletsGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<AccountAbstractionWalletsGrowthMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_account_abstraction_wallets_growth() {
        simple_test_chart::<AccountAbstractionWalletsGrowth>(
            "update_account_abstraction_wallets_growth",
            vec![("2022-11-09", "1")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_account_abstraction_wallets_growth_weekly() {
        simple_test_chart::<AccountAbstractionWalletsGrowthWeekly>(
            "update_account_abstraction_wallets_growth_weekly",
            vec![("2022-11-07", "1")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_account_abstraction_wallets_growth_monthly() {
        simple_test_chart::<AccountAbstractionWalletsGrowthMonthly>(
            "update_account_abstraction_wallets_growth_monthly",
            vec![("2022-11-01", "1")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_account_abstraction_wallets_growth_yearly() {
        simple_test_chart::<AccountAbstractionWalletsGrowthYearly>(
            "update_account_abstraction_wallets_growth_yearly",
            vec![("2022-01-01", "1")],
        )
        .await;
    }
}
