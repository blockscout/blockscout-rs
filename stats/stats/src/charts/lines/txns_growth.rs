use crate::data_source::kinds::updateable_chart::cumulative::CumulativeChartWrapper;

/// Items in this module are not intended to be used outside. They are only public
/// since the actual public type is just an alias (to wrapper).
///
/// I.e. use [`super`]'s types.
pub mod _inner {

    use crate::{
        charts::{chart::Chart, db_interaction::types::DateValueInt},
        data_source::kinds::updateable_chart::cumulative::CumulativeChart,
        lines::NewTxnsInt,
        MissingDatePolicy, Named,
    };
    use entity::sea_orm_active_enums::ChartType;

    pub struct TxnsGrowthInner;

    impl Named for TxnsGrowthInner {
        const NAME: &'static str = "txnsGrowth";
    }

    impl Chart for TxnsGrowthInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
        fn missing_date_policy() -> MissingDatePolicy {
            MissingDatePolicy::FillPrevious
        }
    }

    impl CumulativeChart for TxnsGrowthInner {
        type DeltaChartPoint = DateValueInt;
        type DeltaChart = NewTxnsInt;
    }
}
pub type TxnsGrowth = CumulativeChartWrapper<_inner::TxnsGrowthInner>;

#[cfg(test)]
mod tests {
    use super::TxnsGrowth;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth() {
        simple_test_chart::<TxnsGrowth>(
            "update_txns_growth",
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "17"),
                ("2022-11-11", "31"),
                ("2022-11-12", "36"),
                ("2022-12-01", "41"),
                ("2023-01-01", "42"),
                ("2023-02-01", "46"),
                ("2023-03-01", "47"),
            ],
        )
        .await;
    }
}
