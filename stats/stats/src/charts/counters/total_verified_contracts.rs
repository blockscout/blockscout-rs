use crate::{
    data_source::kinds::updateable_chart::last_point::{LastPointChart, LastPointChartWrapper},
    lines::VerifiedContractsGrowth,
    Chart, Named,
};
use entity::sea_orm_active_enums::ChartType;

pub struct TotalVerifiedContractsInner;

impl Named for TotalVerifiedContractsInner {
    const NAME: &'static str = "totalVerifiedContracts";
}

impl Chart for TotalVerifiedContractsInner {
    fn chart_type() -> ChartType {
        ChartType::Counter
    }
}

impl LastPointChart for TotalVerifiedContractsInner {
    type InnerSource = VerifiedContractsGrowth;
}

pub type TotalVerifiedContracts = LastPointChartWrapper<TotalVerifiedContractsInner>;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_verified_contracts() {
        simple_test_counter::<TotalVerifiedContracts>("update_total_verified_contracts", "3").await;
    }
}
