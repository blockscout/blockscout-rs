use crate::data_source::kinds::updateable_chart::last_point::LastPointChartWrapper;

mod _inner {
    use crate::{
        data_source::kinds::updateable_chart::last_point::LastPointChart, lines::NewContracts,
        Chart, Named,
    };
    use entity::sea_orm_active_enums::ChartType;

    pub struct LastNewContractsInner;

    impl Named for LastNewContractsInner {
        const NAME: &'static str = "lastNewContracts";
    }

    impl Chart for LastNewContractsInner {
        fn chart_type() -> ChartType {
            ChartType::Counter
        }
        fn relevant_or_zero() -> bool {
            true
        }
    }

    impl LastPointChart for LastNewContractsInner {
        type InnerSource = NewContracts;
    }
}

pub type LastNewContracts = LastPointChartWrapper<_inner::LastNewContractsInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_contracts() {
        simple_test_counter::<LastNewContracts>("update_last_new_contracts", "0", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_contracts_update_before_new() {
        simple_test_counter::<LastNewContracts>(
            "update_last_new_contracts_update_before_new",
            "0",
            Some(dt("2023-02-01T09:00:00")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_contracts_update_after_new() {
        simple_test_counter::<LastNewContracts>(
            "update_last_new_contracts_update_after_new",
            "1",
            Some(dt("2023-02-01T10:00:01")),
        )
        .await;
    }
}
