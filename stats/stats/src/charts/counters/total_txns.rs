use crate::{
    data_source::kinds::updateable_chart::sum_point::{SumPointChart, SumPointChartWrapper},
    lines::NewTxnsInt,
    Chart, Named,
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

pub type TotalTxns = SumPointChartWrapper<TotalTxnsInner>;

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
