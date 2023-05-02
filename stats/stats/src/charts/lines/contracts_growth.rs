use super::NewContracts;
use crate::{
    charts::{
        chart::Chart,
        create_chart,
        insert::DateValue,
        updater::{parse_and_growth, ChartDependentUpdater},
    },
    UpdateError,
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
        parse_and_growth::<i64>(parent_data, self.parent.name())
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

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        self.parent.create(db).await?;
        create_chart(db, self.name().into(), self.chart_type()).await
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, force_full).await?;
        Ok(())
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
                ("2022-11-09", "2"),
                ("2022-11-10", "5"),
                ("2022-11-11", "7"),
            ],
        )
        .await;
    }
}
