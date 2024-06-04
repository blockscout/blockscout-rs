//! Last point data source.
//!
//! Takes last data point from some other source

use std::marker::PhantomData;

use blockscout_metrics_tools::AggregateTimer;
use tracing::warn;

use crate::{
    charts::{
        chart::{chart_portrait, Chart},
        db_interaction::write::insert_data_many,
    },
    data_source::{source::DataSource, types::UpdateContext},
    utils::day_start,
    DateValueString, Named, UpdateError,
};

use super::{UpdateableChart, UpdateableChartWrapper};

/// See [module-level documentation](self) for details.
pub trait LastPointSource {
    type InnerSource: DataSource<Output = Vec<DateValueString>>;
}

/// Wrapper to convert type implementing [`LastPointSource`] to another that implements [`DataSource`]
pub type LastPointSourceWrapper<T> = UpdateableChartWrapper<LastPointSourceLocalWrapper<T>>;

/// Wrapper to get type implementing "parent" trait. Use [`DeltaChartWrapper`] to get [`DataSource`]
pub struct LastPointSourceLocalWrapper<T: LastPointSource>(PhantomData<T>);

impl<T: LastPointSource + Named> Named for LastPointSourceLocalWrapper<T> {
    const NAME: &'static str = T::NAME;
}

#[portrait::fill(portrait::delegate(T))]
impl<T: LastPointSource + Chart> Chart for LastPointSourceLocalWrapper<T> {}

impl<T: LastPointSource + Chart> UpdateableChart for LastPointSourceLocalWrapper<T> {
    type PrimaryDependency = T::InnerSource;
    type SecondaryDependencies = ();
    type Point = DateValueString;

    async fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        _last_accurate_point: Option<DateValueString>,
        min_blockscout_block: i64,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        let data = Self::PrimaryDependency::query_data(
            cx,
            Some(day_start(cx.time.date_naive())..=cx.time),
            remote_fetch_timer,
        )
        .await?;
        let Some(last_point) = data.last() else {
            warn!(
                chart = Self::NAME,
                "dependency did not return any points; skipping the update"
            );
            return Ok(());
        };
        let last_point = last_point.active_model(chart_id, Some(min_blockscout_block));
        insert_data_many(cx.db, vec![last_point])
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}
