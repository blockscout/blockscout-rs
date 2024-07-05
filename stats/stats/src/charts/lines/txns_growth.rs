use crate::{
    charts::chart::ChartProperties, data_source::kinds::local_db::DailyCumulativeLocalDbChartSource,
    lines::NewTxnsInt, MissingDatePolicy, Named,
};
use entity::sea_orm_active_enums::ChartType;

pub struct TxnsGrowthProperties;

impl Named for TxnsGrowthProperties {
    const NAME: &'static str = "txnsGrowth";
}

impl ChartProperties for TxnsGrowthProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type TxnsGrowth = DailyCumulativeLocalDbChartSource<NewTxnsInt, TxnsGrowthProperties>;

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
