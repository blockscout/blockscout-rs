//! Only retrieves new values and updates the latest one.
//!
//! In some cases performes full update (i.e. when some inconsistency was found or `force_full` is set)

use super::{get_last_row, get_min_block_blockscout};
use crate::{
    charts::{
        db_interaction::{insert::insert_data_many, types::DateValue},
        find_chart,
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
        let chart_id = find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let offset = Some(self.approximate_trailing_values());
        let last_row =
            get_last_row(self, chart_id, min_blockscout_block, db, force_full, offset).await?;
        let values = {
            let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[self.name()])
                .start_timer();
            self.get_values(blockscout, last_row)
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
