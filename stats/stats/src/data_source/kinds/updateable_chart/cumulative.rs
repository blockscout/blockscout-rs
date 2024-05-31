//! Chart that accumulates values of another chart.
//!
//! For example, chart "Total accounts" is a cumulative
//! of "New accounts".
//!
//! So, if the values of `NewItemsChart` are [1, 2, 3, 4], then
//! cumulative chart will produce [1, 3, 6, 10].
//!
//! The opposite of this chart is [delta chart](super::delta).

use std::{fmt::Display, marker::PhantomData, ops::AddAssign, str::FromStr};

use blockscout_metrics_tools::AggregateTimer;
use chrono::Days;

use crate::{
    charts::{
        chart::{chart_portrait, Point},
        db_interaction::{types::DateValue, write::insert_data_many},
    },
    data_processing::cumsum,
    data_source::{DataSource, UpdateContext},
    utils::day_start,
    Chart, DateValueString, Named, UpdateError,
};

use super::{UpdateableChart, UpdateableChartWrapper};

/// See [module-level documentation](self) for details.
pub trait CumulativeChart: Chart {
    type DeltaChartPoint: Point + Default;
    type DeltaChart: DataSource<Output = Vec<Self::DeltaChartPoint>>;
}

/// Wrapper to convert type implementing [`CumulativeChart`] to another that implements [`DataSource`]
pub type CumulativeChartWrapper<T> = UpdateableChartWrapper<CumulativeChartLocalWrapper<T>>;

/// Wrapper to get type implementing "parent" trait. Use [`CumulativeChartWrapper`] to get [`DataSource`]
pub struct CumulativeChartLocalWrapper<T: CumulativeChart>(PhantomData<T>);

impl<T: CumulativeChart + Named> Named for CumulativeChartLocalWrapper<T> {
    const NAME: &'static str = T::NAME;
}

#[portrait::fill(portrait::delegate(T))]
impl<T: CumulativeChart + Chart> Chart for CumulativeChartLocalWrapper<T> {}

impl<T> UpdateableChart for CumulativeChartLocalWrapper<T>
where
    T: CumulativeChart,
    T::DeltaChartPoint: Into<DateValueString>,
    <T::DeltaChartPoint as DateValue>::Value:
        Send + Sync + AddAssign + FromStr + Default + Display + Clone,
    <<T::DeltaChartPoint as DateValue>::Value as FromStr>::Err: Display,
{
    type PrimaryDependency = T::DeltaChart;
    type SecondaryDependencies = ();
    type Point = T::DeltaChartPoint;

    async fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        last_accurate_point: Option<DateValueString>,
        min_blockscout_block: i64,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        let range = last_accurate_point
            .clone()
            .map(|p| day_start(p.date + Days::new(1))..=cx.time);
        let delta_data: Vec<T::DeltaChartPoint> =
            <T::DeltaChart as DataSource>::query_data(cx, range, remote_fetch_timer).await?;
        let partial_sum = last_accurate_point
            .map(|p| {
                p.value
                    .parse::<<T::DeltaChartPoint as DateValue>::Value>()
                    .map_err(|e| {
                        UpdateError::Internal(format!(
                            "failed to parse value in chart '{}': {e}",
                            <Self as Named>::NAME
                        ))
                    })
            })
            .transpose()?;
        let partial_sum = partial_sum.unwrap_or_default();
        let data = cumsum::<T::DeltaChartPoint>(delta_data, partial_sum)?
            .into_iter()
            .map(|value| {
                <T::DeltaChartPoint as Into<DateValueString>>::into(value)
                    .active_model(chart_id, Some(min_blockscout_block))
            });
        insert_data_many(cx.db, data)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}
