//! Cumulative total number of accounts in the network.

use crate::data_source::kinds::updateable_chart::cumulative::CumulativeChartWrapper;

mod _inner {
    use crate::{
        charts::db_interaction::types::DateValueInt,
        data_source::kinds::updateable_chart::cumulative::CumulativeChart,
        lines::new_accounts::NewAccountsInt, Chart, MissingDatePolicy, Named,
    };
    use entity::sea_orm_active_enums::ChartType;

    pub struct AccountsGrowthInner;

    impl Named for AccountsGrowthInner {
        const NAME: &'static str = "accountsGrowth";
    }

    impl Chart for AccountsGrowthInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
        fn missing_date_policy() -> MissingDatePolicy {
            MissingDatePolicy::FillPrevious
        }
    }

    impl CumulativeChart for AccountsGrowthInner {
        type DeltaChart = NewAccountsInt;
        type DeltaChartPoint = DateValueInt;
    }
}

pub type AccountsGrowth = CumulativeChartWrapper<_inner::AccountsGrowthInner>;

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
