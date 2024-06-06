use crate::data_source::kinds::updateable_chart::sum_point::SumPointChartWrapper;

/// Items in this module are not intended to be used outside. They are only public
/// since the actual public type is just an alias (to wrapper).
///
/// I.e. use [`super`]'s types.
pub mod _inner {
    use crate::{
        data_source::kinds::updateable_chart::sum_point::SumPointChart, lines::NewTxnsInt, Chart,
        Named,
    };
    use entity::sea_orm_active_enums::ChartType;

    pub struct TotalTxnsInner;

    impl Named for TotalTxnsInner {
        const NAME: &'static str = "totalTxns";
    }

    impl Chart for TotalTxnsInner {
        fn chart_type() -> ChartType {
            ChartType::Counter
        }
    }

    impl SumPointChart for TotalTxnsInner {
        type InnerSource = NewTxnsInt;
    }
}

pub type TotalTxns = SumPointChartWrapper<_inner::TotalTxnsInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_txns() {
        simple_test_counter::<TotalTxns>("update_total_txns", "47", None).await;
    }
}
