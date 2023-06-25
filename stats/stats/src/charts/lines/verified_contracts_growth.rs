use super::NewVerifiedContracts;
use crate::{
    charts::{
        chart::Chart,
        create_chart,
        insert::DateValue,
        updater::{parse_and_growth, ChartDependentUpdater},
    },
    MissingDatePolicy, UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct VerifiedContractsGrowth {
    parent: Arc<NewVerifiedContracts>,
}

impl VerifiedContractsGrowth {
    pub fn new(parent: Arc<NewVerifiedContracts>) -> Self {
        Self { parent }
    }
}

#[async_trait]
impl ChartDependentUpdater<NewVerifiedContracts> for VerifiedContractsGrowth {
    fn parent(&self) -> Arc<NewVerifiedContracts> {
        self.parent.clone()
    }

    async fn get_values(&self, parent_data: Vec<DateValue>) -> Result<Vec<DateValue>, UpdateError> {
        parse_and_growth::<i64>(parent_data, self.parent.name())
    }
}

#[async_trait]
impl crate::Chart for VerifiedContractsGrowth {
    fn name(&self) -> &str {
        "verifiedContractsGrowth"
    }
    fn chart_type(&self) -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy(&self) -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
    fn drop_last_point(&self) -> bool {
        false
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
    async fn update_verified_contracts_growth() {
        let chart = VerifiedContractsGrowth::default();
        simple_test_chart(
            "update_verified_contracts_growth",
            chart,
            vec![
                ("2022-11-14", "1"),
                ("2022-11-15", "2"),
                ("2022-11-16", "3"),
            ],
        )
        .await;
    }
}
