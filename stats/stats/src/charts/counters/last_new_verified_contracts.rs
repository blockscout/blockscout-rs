use crate::{
    charts::{
        create_chart,
        insert::DateValue,
        updater::{last_point, ChartDependentUpdater},
    },
    lines::NewVerifiedContracts,
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use std::sync::Arc;

#[derive(Default)]
pub struct LastNewVerifiedContracts {
    parent: Arc<NewVerifiedContracts>,
}

impl LastNewVerifiedContracts {
    pub fn new(parent: Arc<NewVerifiedContracts>) -> Self {
        Self { parent }
    }
}

#[async_trait]
impl ChartDependentUpdater<NewVerifiedContracts> for LastNewVerifiedContracts {
    fn parent(&self) -> Arc<NewVerifiedContracts> {
        self.parent.clone()
    }

    async fn get_values(&self, parent_data: Vec<DateValue>) -> Result<Vec<DateValue>, UpdateError> {
        let last = last_point(parent_data);
        Ok(last.into_iter().collect())
    }
}

#[async_trait]
impl crate::Chart for LastNewVerifiedContracts {
    fn name(&self) -> &str {
        "lastNewVerifiedContracts"
    }
    fn chart_type(&self) -> ChartType {
        ChartType::Counter
    }
    fn relevant_or_zero(&self) -> bool {
        true
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
    async fn update_last_new_verified_contracts() {
        let counter = LastNewVerifiedContracts::default();
        simple_test_counter("update_last_new_verified_contracts", counter, "1").await;
    }
}
