use crate::{
    charts::{create_chart, insert::DateValue, updater::ChartDependentUpdater},
    lines::NativeCoinHoldersGrowth,
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use std::sync::Arc;

#[derive(Default)]
pub struct TotalNativeCoinHolders {
    parent: Arc<NativeCoinHoldersGrowth>,
}

impl TotalNativeCoinHolders {
    pub fn new(parent: Arc<NativeCoinHoldersGrowth>) -> Self {
        Self { parent }
    }
}

#[async_trait]
impl ChartDependentUpdater<NativeCoinHoldersGrowth> for TotalNativeCoinHolders {
    fn parent(&self) -> Arc<NativeCoinHoldersGrowth> {
        self.parent.clone()
    }

    async fn get_values(&self, parent_data: Vec<DateValue>) -> Result<Vec<DateValue>, UpdateError> {
        Ok(parent_data.into_iter().max().into_iter().collect())
    }
}

#[async_trait]
impl crate::Chart for TotalNativeCoinHolders {
    fn name(&self) -> &str {
        "totalNativeCoinHolders"
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Counter
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
        self.update_with_values(db, blockscout, force_full).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_native_coin_holders() {
        let counter = TotalNativeCoinHolders::default();
        simple_test_counter("update_total_native_coin_holders", counter, "7").await;
    }
}
