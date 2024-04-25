use crate::{
    charts::{
        create_chart,
        db_interaction::{
            chart_updaters::{last_point, ChartDependentUpdater, ChartUpdater},
            types::DateValue,
        },
    },
    lines::VerifiedContractsGrowth,
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use std::sync::Arc;

#[derive(Default)]
pub struct TotalVerifiedContracts {
    parent: Arc<VerifiedContractsGrowth>,
}

impl TotalVerifiedContracts {
    pub fn new(parent: Arc<VerifiedContractsGrowth>) -> Self {
        Self { parent }
    }
}

#[async_trait]
impl ChartDependentUpdater<VerifiedContractsGrowth> for TotalVerifiedContracts {
    fn parent(&self) -> Arc<VerifiedContractsGrowth> {
        self.parent.clone()
    }

    async fn get_values(&self, parent_data: Vec<DateValue>) -> Result<Vec<DateValue>, UpdateError> {
        let last = last_point(parent_data);
        Ok(last.into_iter().collect())
    }
}

#[async_trait]
impl crate::Chart for TotalVerifiedContracts {
    fn name(&self) -> &str {
        "totalVerifiedContracts"
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Counter
    }

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        self.parent.create(db).await?;
        create_chart(db, self.name().into(), self.chart_type()).await
    }
}

#[async_trait]
impl ChartUpdater for TotalVerifiedContracts {
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
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_verified_contracts() {
        let counter = TotalVerifiedContracts::default();
        simple_test_counter("update_total_verified_contracts", counter, "3").await;
    }
}
