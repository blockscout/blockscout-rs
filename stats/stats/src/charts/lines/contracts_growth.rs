use crate::{
    data_source::kinds::local_db::DailyCumulativeLocalDbChartSource,
    lines::new_contracts::NewContractsInt, ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct ContractsGrowthProperties;

impl Named for ContractsGrowthProperties {
    fn name() -> String {
        "contractsGrowth".into()
    }
}

impl ChartProperties for ContractsGrowthProperties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type ContractsGrowth =
    DailyCumulativeLocalDbChartSource<NewContractsInt, ContractsGrowthProperties>;

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
