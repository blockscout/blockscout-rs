//! Update logic for charts.
//!
//! Depending on the chart nature, various tactics are better fit (in terms of efficiency, performance, etc.).

use crate::{
    charts::{
        data_source::{ChartMetadata, UpdateContext, UpdateParameters},
        find_chart,
        mutex::get_global_update_mutex,
    },
    Chart, DateValue, UpdateError,
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

pub trait ChartUpdater: Chart {
    /// Update only data (values) of the chart (`chart_data` table).
    ///
    /// Implementation is expected to be highly variable.
    async fn update_values(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Vec<DateValue>, UpdateError>;

    /// Update only metadata of the chart (`charts` table).
    ///
    /// Generally better to call after changing chart data to keep
    /// the info relevant (i.e. if it depends on values).
    async fn update_metadata(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<ChartMetadata, UpdateError> {
        let cx = &cx.user_context;
        let UpdateParameters {
            db, current_time, ..
        } = cx;
        let chart_id = find_chart(db, Self::name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;
        common_operations::set_last_updated_at(chart_id, db, current_time.clone())
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(ChartMetadata {
            last_update: current_time.clone(),
        })
    }

    /// Update data and metadata of the chart.
    ///
    /// `current_time` is settable mainly for testing purposes. So that
    /// code dependant on time (mostly metadata updates) can be reproducibly tested.
    async fn update(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(Vec<DateValue>, ChartMetadata), UpdateError> {
        Ok((
            Self::update_values(cx).await?,
            Self::update_metadata(cx).await?,
        ))
    }

    /// Run [`Self::update`] with acquiring global mutex for the chart
    async fn update_with_mutex(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(Vec<DateValue>, ChartMetadata), UpdateError> {
        let name = Self::name();
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
        Self::update(cx).await
    }
}
