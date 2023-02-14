use crate::{
    charts::{
        find_chart,
        insert::{insert_data_many, DateValue},
    },
    metrics, Chart, UpdateError,
};
use async_trait::async_trait;
use sea_orm::prelude::*;

#[async_trait]
pub trait ChartFullUpdater: Chart {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError>;

    async fn update_with_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        _force_full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let values = {
            let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[self.name()])
                .start_timer();
            self.get_values(blockscout)
                .await?
                .into_iter()
                .map(|value| value.active_model(chart_id, None))
        };
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        Ok(())
    }
}
