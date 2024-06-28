use crate::{
    data_source::kinds::{
        data_manipulation::last_point::LastPoint, local_db::DirectPointLocalDbChartSource,
    },
    lines::VerifiedContractsGrowth,
    ChartProperties, MissingDatePolicy, Named,
};

use entity::sea_orm_active_enums::ChartType;

pub struct TotalVerifiedContractsProperties;

impl Named for TotalVerifiedContractsProperties {
    const NAME: &'static str = "totalVerifiedContracts";
}

impl ChartProperties for TotalVerifiedContractsProperties {
    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type TotalVerifiedContracts = DirectPointLocalDbChartSource<
    LastPoint<VerifiedContractsGrowth>,
    TotalVerifiedContractsProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_verified_contracts() {
        simple_test_counter::<TotalVerifiedContracts>("update_total_verified_contracts", "3", None)
            .await;
    }
}
