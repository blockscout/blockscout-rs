//! The opposite of [cumulative chart](super::cumulative).
//!
//! I.e. chart "New accounts" is a delta
//! of  "Total accounts".

use std::{fmt::Display, marker::PhantomData, ops::SubAssign, str::FromStr};

use blockscout_metrics_tools::AggregateTimer;

use crate::{
    charts::{
        chart::{chart_portrait, Point},
        db_interaction::{types::DateValue, write::insert_data_many},
    },
    data_processing::deltas,
    data_source::{DataSource, UpdateContext},
    utils::day_start,
    Chart, DateValueString, Named, UpdateError,
};

use super::{UpdateableChart, UpdateableChartWrapper};

/// See [module-level documentation](self) for details.
pub trait DeltaChart: Chart {
    type CumulativeChartPoint: Point + Default;
    type CumulativeChart: DataSource<Output = Vec<Self::CumulativeChartPoint>>;
}

/// Wrapper to convert type implementing [`DeltaChart`] to another that implements [`DataSource`]
pub type DeltaChartWrapper<T> = UpdateableChartWrapper<DeltaChartLocalWrapper<T>>;

/// Wrapper to get type implementing "parent" trait. Use [`DeltaChartWrapper`] to get [`DataSource`]
pub struct DeltaChartLocalWrapper<T: DeltaChart>(PhantomData<T>);

impl<T: DeltaChart + Named> Named for DeltaChartLocalWrapper<T> {
    const NAME: &'static str = T::NAME;
}

#[portrait::fill(portrait::delegate(T))]
impl<T: DeltaChart + Chart> Chart for DeltaChartLocalWrapper<T> {}

impl<T> UpdateableChart for DeltaChartLocalWrapper<T>
where
    T: DeltaChart,
    T::CumulativeChartPoint: Into<DateValueString>,
    <T::CumulativeChartPoint as DateValue>::Value:
        Send + Sync + SubAssign + FromStr + Default + Display + Clone,
    <<T::CumulativeChartPoint as DateValue>::Value as FromStr>::Err: Display,
{
    type PrimaryDependency = T::CumulativeChart;
    type SecondaryDependencies = ();

    async fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        last_accurate_point: Option<DateValueString>,
        min_blockscout_block: i64,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        let range = last_accurate_point
            .clone()
            // no additional day because we need one point before the range to calculate
            .map(|p| day_start(p.date)..=cx.time);
        let cum_data: Vec<T::CumulativeChartPoint> =
            <T::CumulativeChart as DataSource>::query_data(cx, range, remote_fetch_timer).await?;
        let mut cum_data = cum_data.into_iter();
        let Some(range_start) = cum_data.next() else {
            // todo: check what happens with missing points (and when they do occur)
            // also check if filling the points in query_data is needed (probably yes)
            // todo: maybe additional data structure that stores lazy data but allows lookup
            tracing::warn!(
                // todo: chart name and other tracing info
                "Value before the range was not found, finishing update"
            );
            return Ok(());
        };
        if let Some(p) = last_accurate_point {
            if range_start.get_parts().0 != &p.date {
                tracing::warn!(
                    // todo: chart name and other tracing info
                    "Unexpected first point date, this might be a reason for inaccurate data \
                    after the update."
                );
            }
        }
        let range_start_value = range_start.into_parts().1;
        let data = deltas::<T::CumulativeChartPoint>(cum_data.collect(), range_start_value)?
            .into_iter()
            .map(|value| {
                <T::CumulativeChartPoint as Into<DateValueString>>::into(value)
                    .active_model(chart_id, Some(min_blockscout_block))
            });
        insert_data_many(cx.db, data)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}
