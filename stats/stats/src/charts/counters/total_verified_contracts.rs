use crate::data_source::kinds::updateable_chart::last_point::LastPointChartWrapper;

/// Items in this module are not intended to be used outside. They are only public
/// since the actual public type is just an alias (to wrapper).
///
/// I.e. use [`super`]'s types.
pub mod _inner {
    use crate::{
        data_source::kinds::updateable_chart::last_point::LastPointChart,
        lines::VerifiedContractsGrowth, Chart, Named,
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
}

pub type TotalVerifiedContracts = LastPointChartWrapper<_inner::TotalVerifiedContractsInner>;

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
