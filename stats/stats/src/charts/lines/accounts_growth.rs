//! Cumulative total number of accounts in the network.

use super::new_accounts::NewAccountsInt;
use crate::{
    data_source::kinds::{
        data_manipulation::resolutions::last_value::LastValueLowerResolution,
        local_db::{
            parameters::update::batching::parameters::{Batch30Weeks, Batch30Years, Batch36Months},
            DailyCumulativeLocalDbChartSource, DirectVecLocalDbChartSource,
        },
    },
    delegated_properties_with_resolutions,
    types::timespans::{Month, Week, Year},
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "accountsGrowth".into()
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

delegated_properties_with_resolutions!(
    delegate: {
        WeeklyProperties: Week,
        MonthlyProperties: Month,
        YearlyProperties: Year,
    }
    ..Properties
);

pub type AccountsGrowth = DailyCumulativeLocalDbChartSource<NewAccountsInt, Properties>;
pub type AccountsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<AccountsGrowth, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AccountsGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<AccountsGrowth, Month>,
    Batch36Months,
    MonthlyProperties,
>;
pub type AccountsGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<AccountsGrowthMonthly, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth() {
        simple_test_chart::<AccountsGrowth>(
            "update_accounts_growth",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "4"),
                ("2022-11-11", "8"),
                ("2023-03-01", "9"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth_weekly() {
        simple_test_chart::<AccountsGrowthWeekly>(
            "update_accounts_growth_weekly",
            vec![("2022-11-07", "8"), ("2023-02-27", "9")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth_monthly() {
        simple_test_chart::<AccountsGrowthMonthly>(
            "update_accounts_growth_monthly",
            vec![("2022-11-01", "8"), ("2023-03-01", "9")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth_yearly() {
        simple_test_chart::<AccountsGrowthYearly>(
            "update_accounts_growth_yearly",
            vec![("2022-01-01", "8"), ("2023-01-01", "9")],
        )
        .await;
    }
}
