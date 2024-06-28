use crate::{
    data_source::kinds::{
        data_manipulation::{map::MapToString, sum_point::Sum},
        local_db::DirectPointLocalDbChartSource,
    },
    lines::NewTxnsInt,
    ChartProperties, MissingDatePolicy, Named,
};
use entity::sea_orm_active_enums::ChartType;

pub struct TotalTxnsProperties;

impl Named for TotalTxnsProperties {
    const NAME: &'static str = "totalTxns";
}

impl ChartProperties for TotalTxnsProperties {
    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type TotalTxns =
    DirectPointLocalDbChartSource<MapToString<Sum<NewTxnsInt>>, TotalTxnsProperties>;

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
