use crate::{
    data_source::kinds::{
        data_manipulation::last_point::LastPoint, local_db::DirectPointLocalDbChartSource,
    },
    lines::NewVerifiedContracts,
    ChartProperties, Named,
};

use entity::sea_orm_active_enums::ChartType;

pub struct LastNewVerifiedContractsProperties;

impl Named for LastNewVerifiedContractsProperties {
    const NAME: &'static str = "lastNewVerifiedContracts";
}

impl ChartProperties for LastNewVerifiedContractsProperties {
    fn chart_type() -> ChartType {
        ChartType::Counter
    }
}

pub type LastNewVerifiedContracts = DirectPointLocalDbChartSource<
    LastPoint<NewVerifiedContracts>,
    LastNewVerifiedContractsProperties,
>;

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
