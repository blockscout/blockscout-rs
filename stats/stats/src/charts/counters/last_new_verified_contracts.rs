use crate::{
    data_source::kinds::updateable_chart::last_point::{LastPointChart, LastPointChartWrapper},
    lines::NewVerifiedContracts,
    Chart, Named,
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

pub type LastNewVerifiedContracts = LastPointChartWrapper<LastNewVerifiedContractsInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_verified_contracts() {
        simple_test_counter::<LastNewVerifiedContracts>("update_last_new_verified_contracts", "1")
            .await;
    }
}
