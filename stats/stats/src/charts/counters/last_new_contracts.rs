use crate::{
    data_source::kinds::updateable_chart::last_point::{LastPointChart, LastPointChartWrapper},
    lines::NewContracts,
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

pub type LastNewContracts = LastPointChartWrapper<LastNewContractsInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_contracts() {
        simple_test_counter::<LastNewContracts>("update_last_new_contracts", "1").await;
    }
}
