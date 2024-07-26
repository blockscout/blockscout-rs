//! Cumulative total number of accounts in the network.

use super::new_accounts::NewAccountsInt;
use crate::{
    data_source::kinds::{
        data_manipulation::resolutions::last_value::LastValueLowerResolution,
        local_db::{
            parameters::update::batching::parameters::Batch30Weeks,
            DailyCumulativeLocalDbChartSource, DirectVecLocalDbChartSource,
        },
    },
    delegated_property_with_resolution,
    types::week::Week,
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

delegated_property_with_resolution!(WeeklyProperties {
    resolution: Week,
    ..Properties
});

pub type AccountsGrowth = DailyCumulativeLocalDbChartSource<NewAccountsInt, Properties>;

pub type AccountsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<AccountsGrowth, Week>,
    Batch30Weeks,
    WeeklyProperties,
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
}
