//! Chart that accumulates values of another chart.
//!
//! For example, chart "Total accounts" is a cumulative
//! of "New accounts".
//!
//! So, if the values of `NewItemsChart` are [1, 2, 3, 4], then
//! cumulative chart will produce [1, 3, 6, 10].
//!
//! The opposite of this chart is [delta chart](super::delta).

use crate::{charts::chart::Point, data_source::DataSource, Chart};

use super::{UpdateableChart, UpdateableChartWrapper};

/// See [module-level documentation](self) for details.
pub trait CumulativeChart: Chart {
    // only need because couldn't figure out how to "extract" generic
    // T from `Vec` and place bounds on it
    /// Type of elements in the output of [`DeltaChart`](CumulativeChart::DeltaChart)
    type DeltaChartPoint: Point + Default;
    // todo: rename to source
    type DeltaChart: DataSource<Output = Vec<Self::DeltaChartPoint>>;
}

/// Wrapper to convert type implementing [`CumulativeChart`] to another that implements [`DataSource`]
pub type CumulativeChartWrapper<T> = UpdateableChartWrapper<_inner::CumulativeChartLocalWrapper<T>>;

mod _inner {
    use std::{fmt::Display, marker::PhantomData, ops::AddAssign, str::FromStr};

    use blockscout_metrics_tools::AggregateTimer;
    use chrono::Days;
    use rust_decimal::prelude::Zero;

    use crate::{
        charts::{
            chart::chart_portrait,
            db_interaction::{types::DateValue, write::insert_data_many},
        },
        data_processing::cumsum,
        data_source::{DataSource, UpdateContext},
        utils::day_start,
        Chart, DateValueString, Named, UpdateError,
    };

    use super::{CumulativeChart, UpdateableChart};
    /// Wrapper to get type implementing "parent" trait. Use [`super::CumulativeChartWrapper`] to get [`DataSource`]
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
            Send + Sync + AddAssign + FromStr + Display + Clone + Zero,
        <<T::DeltaChartPoint as DateValue>::Value as FromStr>::Err: Display,
    {
        type PrimaryDependency = T::DeltaChart;
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
            let partial_sum =
                partial_sum.unwrap_or(<T::DeltaChartPoint as DateValue>::Value::zero());
            tracing::debug!(
                partial_sum = %partial_sum,
                delta_points_len = delta_data.len(),
                "calculating cumulative sum"
            );
            // it's ok to have missing points
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
}
