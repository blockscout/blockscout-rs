use crate::{
    charts::{
        create_chart, find_chart,
        insert::{insert_data_many, DateValue},
        updater::get_min_block_blockscout,
    },
    get_chart_data,
    lines::NewNativeCoinTransfers,
    Chart, Point, UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use std::sync::Arc;

#[derive(Default)]
pub struct TotalNativeCoinTransfers {
    parent: Arc<NewNativeCoinTransfers>,
}

impl TotalNativeCoinTransfers {
    pub fn new(parent: Arc<NewNativeCoinTransfers>) -> Self {
        Self { parent }
    }

    async fn get_parent_data(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        full: bool,
    ) -> Result<Vec<Point>, UpdateError> {
        tracing::info!(
            chart_name = self.name(),
            parent_chart_name = self.parent.name(),
            "update parent"
        );
        self.parent.update(db, blockscout, full).await?;
        let data = get_chart_data(db, self.parent.name(), None, None).await?;
        Ok(data)
    }
}

#[async_trait]
impl crate::Chart for TotalNativeCoinTransfers {
    fn name(&self) -> &str {
        "totalNativeCoinTransfers"
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
        full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let new_transfers_per_day = self.get_parent_data(db, blockscout, full).await?;
        let max_date = match new_transfers_per_day.iter().max() {
            Some(max_date) => max_date.date,
            None => {
                tracing::warn!(
                    chart_name = self.name(),
                    parent_chart_name = self.parent.name(),
                    "parent doesn't have any data after update"
                );
                return Ok(());
            }
        };
        let total_transfers: i64 = new_transfers_per_day
            .into_iter()
            .map(|p| p.value.parse::<i64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                UpdateError::Internal(format!(
                    "chart {} has invalid data: {}",
                    self.parent.name(),
                    e
                ))
            })?
            .into_iter()
            .sum();
        let point = DateValue {
            date: max_date,
            value: total_transfers.to_string(),
        }
        .active_model(chart_id, Some(min_blockscout_block));
        insert_data_many(db, std::iter::once(point))
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_native_coin_transfers() {
        let counter = TotalNativeCoinTransfers::default();
        simple_test_counter("update_total_native_coin_transfers", counter, "11").await;
    }
}
