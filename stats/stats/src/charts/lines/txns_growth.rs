use super::NewTxns;
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
impl ChartDependentUpdater<NewTxns> for TxnsGrowth {
    fn parent(&self) -> Arc<NewTxns> {
        self.parent.clone()
    }

    async fn get_values(&self, parent_data: Vec<DateValue>) -> Result<Vec<DateValue>, UpdateError> {
        parse_and_cumsum::<i64>(parent_data, self.parent.name())
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
    fn missing_date_policy(&self) -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        self.parent.create(db).await?;
        create_chart(db, self.name().into(), self.chart_type()).await
    }
}

#[async_trait]
impl ChartUpdater for TxnsGrowth {
    async fn update_values(
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
