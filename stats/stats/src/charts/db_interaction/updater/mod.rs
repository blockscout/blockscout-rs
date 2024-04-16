//! Update logic for charts.
//!
//! Depending on the chart nature, various tactics are better fit (in terms of efficiency, performance, etc.).

use async_trait::async_trait;
use sea_orm::prelude::*;

use crate::{
    charts::{find_chart, mutex::get_global_update_mutex},
    Chart, UpdateError,
};

mod batch;
pub(crate) mod common_operations;
mod dependent;
mod full;
mod partial;

pub use batch::ChartBatchUpdater;
pub use dependent::{last_point, parse_and_cumsum, parse_and_sum, ChartDependentUpdater};
pub use full::ChartFullUpdater;
pub use partial::ChartPartialUpdater;

#[async_trait]
pub trait ChartUpdater: Chart {
    /// Update only data (values) of the chart (`chart_data` table).
    ///
    /// Implementation is expected to be highly variable.
    async fn update_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError>;

    /// Update only metadata of the chart (`charts` table).
    ///
    /// Generally better to call after changing chart data to keep
    /// the info relevant (i.e. if it depends on values).
    async fn update_metadata(
        &self,
        db: &DatabaseConnection,
        _blockscout: &DatabaseConnection,
    ) -> Result<(), UpdateError> {
        // todo: factor out to `update_chart`
        let chart_id = find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let time = chrono::Utc::now();
        common_operations::update::set_last_updated_at(chart_id, db, time)
            .await
            .map_err(UpdateError::StatsDB)
    }

    /// Update data and metadata of the chart
    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_values(db, blockscout, force_full).await?;
        self.update_metadata(db, blockscout).await
    }

    async fn update_with_mutex(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        let name = self.name();
        let mutex = get_global_update_mutex(name).await;
        let _permit = {
            match mutex.try_lock() {
                Ok(v) => v,
                Err(_) => {
                    tracing::warn!(
                        chart_name = name,
                        "found locked update mutex, waiting for unlock"
                    );
                    mutex.lock().await
                }
            }
        };
        self.update(db, blockscout, force_full).await
    }
}
