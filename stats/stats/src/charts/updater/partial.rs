use crate::{
    charts::{
        chart::get_update_info,
        insert::{insert_data_many, DateValue},
    },
    metrics, Chart, UpdateError,
};
use async_trait::async_trait;
use sea_orm::prelude::*;

#[async_trait]
pub trait ChartPartialUpdater: Chart {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        last_row: Option<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError>;

    async fn update_with_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        let update_info = get_update_info(self, db, blockscout, force_full, None).await?;
        let values = {
            let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[self.name()])
                .start_timer();
            self.get_values(blockscout, update_info.last_row)
                .await?
                .into_iter()
                .map(|value| {
                    value.active_model(update_info.chart_id, Some(update_info.min_blockscout_block))
                })
        };
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}
