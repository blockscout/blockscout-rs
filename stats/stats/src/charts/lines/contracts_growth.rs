use super::NewContracts;
use crate::{
    charts::{
        chart::Chart,
        create_chart,
        db_interaction::{
            chart_updaters::{parse_and_cumsum, ChartDependentUpdater, ChartUpdater},
            types::DateValue,
        },
    },
    MissingDatePolicy, UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct ContractsGrowth {
    parent: Arc<NewContracts>,
}

impl ContractsGrowth {
    pub fn new(parent: Arc<NewContracts>) -> Self {
        Self { parent }
    }
}

#[async_trait]
impl ChartDependentUpdater<NewContracts> for ContractsGrowth {
    fn parent(&self) -> Arc<NewContracts> {
        self.parent.clone()
    }

    async fn get_values(&self, parent_data: Vec<DateValue>) -> Result<Vec<DateValue>, UpdateError> {
        parse_and_cumsum::<i64>(parent_data, self.parent.name())
    }
}

#[async_trait]
impl crate::Chart for ContractsGrowth {
    fn name(&self) -> &str {
        "contractsGrowth"
    }
    fn chart_type(&self) -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy(&self) -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        self.parent.create(db).await?;
        create_chart(db, self.name().into(), self.chart_type()).await
    }
}

#[async_trait]
impl ChartUpdater for ContractsGrowth {
    async fn update_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        current_time: chrono::DateTime<chrono::Utc>,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, current_time, force_full)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_contracts_growth() {
        let chart = ContractsGrowth::default();
        simple_test_chart(
            "update_contracts_growth",
            chart,
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
