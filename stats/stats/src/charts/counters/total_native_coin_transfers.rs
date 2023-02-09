use crate::{
    charts::{
        cache::Cache,
        find_chart,
        insert::{insert_data_many, DateValue},
        updater::get_min_block_blockscout,
    },
    lines::NewNativeCoinTransfers,
    UpdateError,
};
use async_trait::async_trait;
use entity::{chart_data, sea_orm_active_enums::ChartType};
use sea_orm::{prelude::*, QueryOrder, QuerySelect};
use tokio::sync::Mutex;

pub struct TotalNativeCoinTransfers {
    cache: Mutex<Cache<Vec<DateValue>>>,
}

impl TotalNativeCoinTransfers {
    pub fn new(cache: Cache<Vec<DateValue>>) -> Self {
        Self {
            cache: Mutex::new(cache),
        }
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

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        _full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let mut data = {
            let mut cache = self.cache.lock().await;
            cache
                .get_or_update(async move {
                    NewNativeCoinTransfers::read_values(blockscout, None).await
                })
                .await?
        };
        data.sort_unstable();
        let min_date = data.first().map(|v| v.date);
        let max_date = data.last().map(|v| v.date);
        let (min_date, max_date) = match (min_date, max_date) {
            (Some(min), Some(max)) => (min, max),
            _ => {
                tracing::warn!("new txns returned empty array");
                return Ok(());
            }
        };

        let last_data: Option<DateValue> = chart_data::Entity::find()
            .column(chart_data::Column::Date)
            .column(chart_data::Column::Value)
            .filter(chart_data::Column::ChartId.eq(chart_id))
            .filter(chart_data::Column::Date.lt(min_date))
            .order_by_desc(chart_data::Column::Date)
            .into_model()
            .one(db)
            .await
            .map_err(UpdateError::StatsDB)?;
        let prev_total = match last_data {
            Some(last_data) => last_data
                .value
                .parse::<i64>()
                .map_err(|e| UpdateError::Internal(e.to_string()))?,
            None => {
                tracing::info!(
                    chart_name = self.name(),
                    "calculating total counter from 0, because no old data was found"
                );
                0
            }
        };
        let data_sum: i64 = data
            .iter()
            .map(|v| v.value.parse::<i64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| UpdateError::Internal(e.to_string()))?
            .into_iter()
            .sum();
        let value = data_sum + prev_total;

        let point = DateValue {
            date: max_date,
            value: value.to_string(),
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
        let counter = TotalNativeCoinTransfers::new(Cache::default());
        simple_test_counter("update_total_native_coin_transfers", counter, "11").await;
    }
}
