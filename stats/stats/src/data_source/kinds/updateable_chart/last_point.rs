//! Last point data source.
//!
//! Takes last data point from some other source

use crate::{charts::chart::Chart, data_source::source::DataSource, DateValueString};

use super::{UpdateableChart, UpdateableChartWrapper};

/// See [module-level documentation](self) for details.
pub trait LastPointChart {
    type InnerSource: DataSource<Output = Vec<DateValueString>> + Chart;
}

/// Wrapper to convert type implementing [`LastPointChart`] to another that implements [`DataSource`]
pub type LastPointChartWrapper<T> = UpdateableChartWrapper<_inner::LastPointChartLocalWrapper<T>>;

mod _inner {
    use std::marker::PhantomData;

    use blockscout_metrics_tools::AggregateTimer;
    use tracing::warn;

    use crate::{
        charts::{
            chart::{chart_portrait, Chart},
            db_interaction::{types::DateValue, write::insert_data_many},
        },
        data_source::{source::DataSource, types::UpdateContext},
        utils::day_start,
        DateValueString, MissingDatePolicy, Named, UpdateError,
    };

    use super::{LastPointChart, UpdateableChart};

    /// Wrapper to get type implementing "parent" trait. Use [`super::LastPointChartWrapper`] to get [`DataSource`]
    pub struct LastPointChartLocalWrapper<T: LastPointChart>(PhantomData<T>);

    impl<T: LastPointChart + Named> Named for LastPointChartLocalWrapper<T> {
        const NAME: &'static str = T::NAME;
    }

    #[portrait::fill(portrait::delegate(T))]
    impl<T: LastPointChart + Chart> Chart for LastPointChartLocalWrapper<T> {}

    impl<T: LastPointChart + Chart> UpdateableChart for LastPointChartLocalWrapper<T> {
        type PrimaryDependency = T::InnerSource;
        type SecondaryDependencies = ();

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
            tracing::debug!("picking last point from dependency");
            let last_point = data.last().cloned().or_else(|| {
                if T::InnerSource::missing_date_policy() == MissingDatePolicy::FillZero {
                    Some(DateValueString::from_parts(
                        cx.time.date_naive(),
                        "0".to_string(),
                    ))
                } else {
                    None
                }
            });
            let Some(last_point) = last_point else {
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
}
