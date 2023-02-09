use super::NewTxns;
use crate::{
    charts::{chart::Chart, create_chart, insert::DateValue, updater::ChartDependentUpdate},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use std::sync::Arc;

#[derive(Debug)]
pub struct TxnsGrowth {
    parent: Arc<NewTxns>,
}

impl TxnsGrowth {
    pub fn new(parent: Arc<NewTxns>) -> Self {
        Self { parent }
    }
}

#[async_trait]
impl ChartDependentUpdate<NewTxns> for TxnsGrowth {
    fn parent(&self) -> Arc<NewTxns> {
        self.parent.clone()
    }

    async fn get_values(
        &self,
        mut parent_data: Vec<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let mut prev_sum = 0;
        for item in parent_data.iter_mut() {
            let value = item.value.parse::<i64>().map_err(|e| {
                UpdateError::Internal(format!(
                    "failed to parse values to int in chart '{}': {}",
                    self.parent.name(),
                    e
                ))
            })?;
            item.value = (value + prev_sum).to_string();
            prev_sum += value;
        }
        Ok(parent_data)
    }
}

#[async_trait]
impl crate::Chart for TxnsGrowth {
    fn name(&self) -> &str {
        "txnsGrowth"
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
    use super::TxnsGrowth;
    use crate::{lines::NewTxns, tests::simple_test::simple_test_chart};
    use std::sync::Arc;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth() {
        let chart = TxnsGrowth::new(Arc::new(NewTxns::default()));
        simple_test_chart(
            "update_txns_growth",
            chart,
            vec![
                ("2022-11-09", "3"),
                ("2022-11-10", "9"),
                ("2022-11-11", "15"),
                ("2022-11-12", "16"),
            ],
        )
        .await;
    }
}
