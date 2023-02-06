use super::NewTxns;
use crate::{
    cache::Cache,
    charts::{
        find_chart,
        insert::{insert_data_many, DateValue},
        updater::{get_last_row, get_min_block_blockscout},
    },
    UpdateError,
};
use async_trait::async_trait;
use entity::{chart_data, sea_orm_active_enums::ChartType};
use sea_orm::{prelude::*, QueryOrder, QuerySelect};
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct TxnsGrowth {
    cache: Mutex<Cache<Vec<DateValue>>>,
}

impl TxnsGrowth {
    pub fn new(cache: Cache<Vec<DateValue>>) -> Self {
        Self {
            cache: Mutex::new(cache),
        }
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
        let last_row = get_last_row(self, chart_id, min_blockscout_block, db, full).await?;
        let mut data = {
            let mut cache = self.cache.lock().await;
            cache
                .get_or_update(async move { NewTxns::read_values(blockscout, last_row).await })
                .await?
        };
        data.sort();
        let min_data = data.first().map(|v| v.date);
        if let Some(to) = min_data {
            let last_data: Option<DateValue> = chart_data::Entity::find()
                .column(chart_data::Column::Date)
                .column(chart_data::Column::Value)
                .filter(chart_data::Column::ChartId.eq(chart_id))
                .filter(chart_data::Column::Date.lt(to))
                .order_by_desc(chart_data::Column::Date)
                .into_model()
                .one(db)
                .await
                .map_err(UpdateError::StatsDB)?;
            let mut starting_sum = match last_data {
                Some(last_data) => last_data
                    .value
                    .parse::<i64>()
                    .map_err(|e| UpdateError::Internal(e.to_string()))?,
                None => {
                    if last_row.is_some() {
                        tracing::warn!(
                            chart_name = self.name(),
                            last_row = ?last_row,
                            "last_row is Some, but chart doesn't have any data until this row"
                        );
                    }
                    0
                }
            };
            for date_value in data.iter_mut() {
                let v = date_value
                    .value
                    .parse::<i64>()
                    .map_err(|e| UpdateError::Internal(e.to_string()))?;
                date_value.value = (v + starting_sum).to_string();
                starting_sum += v;
            }
            let values = data
                .into_iter()
                .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
            insert_data_many(db, values)
                .await
                .map_err(UpdateError::StatsDB)?;
        } else {
            tracing::warn!("new txns returned empty array")
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::TxnsGrowth;
    use crate::{cache::Cache, tests::simple_test::simple_test_chart};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth() {
        let chart = TxnsGrowth::new(Cache::default());
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
