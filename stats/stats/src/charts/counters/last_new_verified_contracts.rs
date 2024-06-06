use crate::data_source::kinds::updateable_chart::last_point::LastPointChartWrapper;

/// Items in this module are not intended to be used outside. They are only public
/// since the actual public type is just an alias (to wrapper).
///
/// I.e. use [`super`]'s types.
pub mod _inner {
    use crate::{
        data_source::kinds::updateable_chart::last_point::LastPointChart,
        lines::NewVerifiedContracts, Chart, Named,
    };
    use entity::sea_orm_active_enums::ChartType;

    pub struct LastNewVerifiedContractsInner;

    impl Named for LastNewVerifiedContractsInner {
        const NAME: &'static str = "lastNewVerifiedContracts";
    }

    impl Chart for LastNewVerifiedContractsInner {
        fn chart_type() -> ChartType {
            ChartType::Counter
        }
        fn relevant_or_zero() -> bool {
            true
        }
    }

    impl LastPointChart for LastNewVerifiedContractsInner {
        type InnerSource = NewVerifiedContracts;
    }
}

pub type LastNewVerifiedContracts = LastPointChartWrapper<_inner::LastNewVerifiedContractsInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_verified_contracts() {
        simple_test_counter::<LastNewVerifiedContracts>(
            "update_last_new_verified_contracts",
            "0",
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_verified_contracts_update_before_new() {
        simple_test_counter::<LastNewVerifiedContracts>(
            "update_last_new_verified_contracts_update_before_new",
            "0",
            Some(dt("2022-11-15T14:59:59")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_verified_contracts_update_after_new() {
        simple_test_counter::<LastNewVerifiedContracts>(
            "update_last_new_verified_contracts_update_after_new",
            "1",
            Some(dt("2022-11-15T15:00:01")),
        )
        .await;
    }
}
