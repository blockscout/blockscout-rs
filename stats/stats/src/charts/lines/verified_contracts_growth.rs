use crate::data_source::kinds::updateable_chart::cumulative::CumulativeChartWrapper;

mod _inner {

    use crate::{
        charts::{chart::Chart, db_interaction::types::DateValueInt},
        data_source::kinds::updateable_chart::cumulative::CumulativeChart,
        lines::new_verified_contracts::NewVerifiedContractsInt,
        MissingDatePolicy, Named,
    };
    use entity::sea_orm_active_enums::ChartType;

    pub struct VerifiedContractsGrowthInner;

    impl Named for VerifiedContractsGrowthInner {
        const NAME: &'static str = "verifiedContractsGrowth";
    }

    impl Chart for VerifiedContractsGrowthInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
        fn missing_date_policy() -> MissingDatePolicy {
            MissingDatePolicy::FillPrevious
        }
    }

    impl CumulativeChart for VerifiedContractsGrowthInner {
        type DeltaChartPoint = DateValueInt;
        type DeltaChart = NewVerifiedContractsInt;
    }
}
pub type VerifiedContractsGrowth = CumulativeChartWrapper<_inner::VerifiedContractsGrowthInner>;

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
