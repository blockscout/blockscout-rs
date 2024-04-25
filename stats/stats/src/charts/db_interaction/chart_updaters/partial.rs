//! Only retrieves new values and updates the latest one.
//!
//! In some cases performes full update (i.e. when some inconsistency was found or `force_full` is set)

use super::{
    common_operations::{get_min_block_blockscout, get_nth_last_row},
    ChartUpdater,
};
use crate::{
    charts::{
        db_interaction::{types::DateValue, write::insert_data_many},
        find_chart,
    },
    metrics, UpdateError,
};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::prelude::*;

#[async_trait]
pub trait ChartPartialUpdater: ChartUpdater {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        last_updated_row: Option<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError>;

    async fn update_with_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        _current_time: chrono::DateTime<Utc>,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let offset = Some(self.approximate_trailing_points());
        let last_updated_row =
            get_nth_last_row(self, chart_id, min_blockscout_block, db, force_full, offset).await?;
        let values = {
            let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[self.name()])
                .start_timer();
            self.get_values(blockscout, last_updated_row)
                .await?
                .into_iter()
                .map(|value| value.active_model(chart_id, Some(min_blockscout_block)))
        };
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}
