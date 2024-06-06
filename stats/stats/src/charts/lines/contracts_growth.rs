use crate::data_source::kinds::updateable_chart::cumulative::CumulativeChartWrapper;

/// Items in this module are not intended to be used outside. They are only public
/// since the actual public type is just an alias (to wrapper).
///
/// I.e. use [`super`]'s types.
pub mod _inner {
    use crate::{
        charts::db_interaction::types::DateValueInt,
        data_source::kinds::updateable_chart::cumulative::CumulativeChart,
        lines::new_contracts::NewContractsInt, Chart, MissingDatePolicy, Named,
    };
    use entity::sea_orm_active_enums::ChartType;

    pub struct ContractsGrowthInner;

    impl Named for ContractsGrowthInner {
        const NAME: &'static str = "contractsGrowth";
    }

    impl Chart for ContractsGrowthInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
        fn missing_date_policy() -> MissingDatePolicy {
            MissingDatePolicy::FillPrevious
        }
    }

    impl CumulativeChart for ContractsGrowthInner {
        type DeltaChart = NewContractsInt;
        type DeltaChartPoint = DateValueInt;
    }
}
pub type ContractsGrowth = CumulativeChartWrapper<_inner::ContractsGrowthInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_contracts_growth() {
        simple_test_chart::<ContractsGrowth>(
            "update_contracts_growth",
            vec![
                ("2022-11-09", "3"),
                ("2022-11-10", "9"),
                ("2022-11-11", "17"),
                ("2022-11-12", "19"),
                ("2022-12-01", "21"),
                ("2023-01-01", "22"),
                ("2023-02-01", "23"),
            ],
        )
        .await;
    }
}
