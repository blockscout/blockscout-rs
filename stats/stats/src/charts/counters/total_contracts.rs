use crate::{
    charts::{
        create_chart,
        db_interaction::{
            chart_updaters::{last_point, ChartDependentUpdater, ChartFullUpdater, ChartUpdater},
            types::DateValue,
        },
    },
    lines::ContractsGrowth,
    UpdateError,
};
use async_trait::async_trait;
use blockscout_db::entity::addresses;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use std::sync::Arc;

#[derive(Default)]
pub struct TotalContracts {
    parent: Arc<ContractsGrowth>,
}

impl TotalContracts {
    pub fn new(parent: Arc<ContractsGrowth>) -> Self {
        Self { parent }
    }
}

#[async_trait]
impl ChartFullUpdater for TotalContracts {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let value = addresses::Entity::find()
            .filter(addresses::Column::ContractCode.is_not_null())
            .count(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let date = chrono::Utc::now().date_naive();
        Ok(vec![DateValue {
            date,
            value: value.to_string(),
        }])
    }
}

#[async_trait]
impl ChartDependentUpdater<ContractsGrowth> for TotalContracts {
    fn parent(&self) -> Arc<ContractsGrowth> {
        self.parent.clone()
    }

    async fn get_values(&self, parent_data: Vec<DateValue>) -> Result<Vec<DateValue>, UpdateError> {
        let last = last_point(parent_data);
        Ok(last.into_iter().collect())
    }
}

#[async_trait]
impl crate::Chart for TotalContracts {
    fn name(&self) -> &str {
        "totalContracts"
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
impl ChartUpdater for TotalContracts {
    async fn update_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        current_time: chrono::DateTime<chrono::Utc>,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        // todo: reconsider once #845 is solved
        // https://github.com/blockscout/blockscout-rs/issues/845
        <Self as ChartFullUpdater>::update_with_values(
            self,
            db,
            blockscout,
            current_time,
            force_full,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_contracts() {
        let counter = TotalContracts::default();
        simple_test_counter("update_total_contracts", counter, "23").await;
    }
}
