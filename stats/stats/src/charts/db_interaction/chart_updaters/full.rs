//! Re-reads/re-calculates whole chart from the source DB.

use super::ChartUpdater;
use crate::{
    charts::{
        db_interaction::{types::DateValue, write::insert_data_many},
        find_chart,
    },
    data_source::types::{UpdateContext, UpdateParameters},
    metrics, UpdateError,
};
use sea_orm::prelude::*;

pub trait ChartFullUpdater: ChartUpdater {
    async fn get_values(blockscout: &DatabaseConnection) -> Result<Vec<DateValue>, UpdateError>;

    async fn update_with_values(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let cx = &cx.user_context;
        let UpdateParameters { db, blockscout, .. } = *cx;
        let chart_id = find_chart(db, Self::NAME)
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(Self::NAME.into()))?;
        let (values, data) = {
            let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[Self::NAME])
                .start_timer();
            let data = Self::get_values(blockscout).await?;
            let values = data
                .clone()
                .into_iter()
                .map(|value| value.active_model(chart_id, None));
            (values, data)
        };
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        Ok(data)
    }
}
