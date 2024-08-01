use crate::{
    data_source::kinds::{
        data_manipulation::resolutions::last_value::LastValueLowerResolution,
        local_db::{
            parameters::update::batching::parameters::{Batch30Weeks, Batch30Years, Batch36Months},
            DailyCumulativeLocalDbChartSource, DirectVecLocalDbChartSource,
        },
    },
    delegated_properties_with_resolutions,
    lines::new_contracts::NewContractsInt,
    types::timespans::{Month, Week, Year},
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "contractsGrowth".into()
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

pub type ContractsGrowth = DailyCumulativeLocalDbChartSource<NewContractsInt, Properties>;
pub type ContractsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<ContractsGrowth, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type ContractsGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<ContractsGrowth, Month>,
    Batch36Months,
    MonthlyProperties,
>;
pub type ContractsGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<ContractsGrowthMonthly, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_contracts_growth() {
        simple_test_chart::<ContractsGrowth>(
            "update_contracts_growth",
            vec![
                ("2022-11-09", "3"),
                ("2022-11-10", "9"),
                ("2022-11-11", "17"),
                ("2022-11-12", "19"),
                ("2022-12-01", "21"),
                ("2023-01-01", "22"),
                ("2023-02-01", "23"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_contracts_growth_weekly() {
        simple_test_chart::<ContractsGrowthWeekly>(
            "update_contracts_growth_weekly",
            vec![
                ("2022-11-07", "19"),
                ("2022-11-28", "21"),
                ("2022-12-26", "22"),
                ("2023-01-30", "23"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_contracts_growth_monthly() {
        simple_test_chart::<ContractsGrowthMonthly>(
            "update_contracts_growth_monthly",
            vec![
                ("2022-11-01", "19"),
                ("2022-12-01", "21"),
                ("2023-01-01", "22"),
                ("2023-02-01", "23"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_contracts_growth_yearly() {
        simple_test_chart::<ContractsGrowthYearly>(
            "update_contracts_growth_yearly",
            vec![("2022-01-01", "21"), ("2023-01-01", "23")],
        )
        .await;
    }
}
