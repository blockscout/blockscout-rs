//! Cumulative total number of accounts in the network.

use super::new_accounts::NewAccountsInt;
use crate::{
    data_source::kinds::local_db::CumulativeLocalDbChartSource, ChartProperties, MissingDatePolicy,
    Named,
};

use entity::sea_orm_active_enums::ChartType;

pub struct AccountsGrowthProperties;

impl Named for AccountsGrowthProperties {
    const NAME: &'static str = "accountsGrowth";
}

impl ChartProperties for AccountsGrowthProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type AccountsGrowth = CumulativeLocalDbChartSource<NewAccountsInt, AccountsGrowthProperties>;

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
