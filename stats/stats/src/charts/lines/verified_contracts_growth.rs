use crate::{
    charts::chart::ChartProperties, data_source::kinds::local_db::CumulativeLocalDbChartSource,
    lines::new_verified_contracts::NewVerifiedContractsInt, MissingDatePolicy, Named,
};
use entity::sea_orm_active_enums::ChartType;

pub struct VerifiedContractsGrowthProperties;

impl Named for VerifiedContractsGrowthProperties {
    const NAME: &'static str = "verifiedContractsGrowth";
}

impl ChartProperties for VerifiedContractsGrowthProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type VerifiedContractsGrowth =
    CumulativeLocalDbChartSource<NewVerifiedContractsInt, VerifiedContractsGrowthProperties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_verified_contracts_growth() {
        simple_test_chart::<VerifiedContractsGrowth>(
            "update_verified_contracts_growth",
            vec![
                ("2022-11-14", "1"),
                ("2022-11-15", "2"),
                ("2022-11-16", "3"),
            ],
        )
        .await;
    }
}
