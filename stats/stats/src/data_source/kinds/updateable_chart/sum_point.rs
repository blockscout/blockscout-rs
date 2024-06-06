//! Sum data source.
//!
//! Sums all points from the other data source.

use crate::{charts::db_interaction::types::DateValueInt, data_source::source::DataSource};

use super::{UpdateableChart, UpdateableChartWrapper};

/// See [module-level documentation](self) for details.
pub trait SumPointChart {
    type InnerSource: DataSource<Output = Vec<DateValueInt>>;
}

/// Wrapper to convert type implementing [`SumPointChart`] to another that implements [`DataSource`]
pub type SumPointChartWrapper<T> = UpdateableChartWrapper<_inner::SumPointChartLocalWrapper<T>>;

mod _inner {
    use std::marker::PhantomData;

    use blockscout_metrics_tools::AggregateTimer;

    use crate::{
        charts::{
            chart::{chart_portrait, Chart},
            db_interaction::{types::DateValueInt, write::insert_data_many},
        },
        data_processing::sum,
        data_source::{source::DataSource, types::UpdateContext},
        DateValueString, Named, UpdateError,
    };

    use super::{SumPointChart, UpdateableChart};

    /// Wrapper to get type implementing "parent" trait. Use [`super::SumPointChartWrapper`] to get [`DataSource`]
    pub struct SumPointChartLocalWrapper<T: SumPointChart>(PhantomData<T>);

    impl<T: SumPointChart + Named> Named for SumPointChartLocalWrapper<T> {
        const NAME: &'static str = T::NAME;
    }

    #[portrait::fill(portrait::delegate(T))]
    impl<T: SumPointChart + Chart> Chart for SumPointChartLocalWrapper<T> {}

    impl<T: SumPointChart + Chart> UpdateableChart for SumPointChartLocalWrapper<T> {
        type PrimaryDependency = T::InnerSource;
        type SecondaryDependencies = ();

        async fn update_values(
            cx: &UpdateContext<'_>,
            chart_id: i32,
            _last_accurate_point: Option<DateValueString>,
            min_blockscout_block: i64,
            remote_fetch_timer: &mut AggregateTimer,
        ) -> Result<(), UpdateError> {
            // it's possible to not request full data range and use `_last_accurate_point`; can be updated if needed
            let full_data =
                Self::PrimaryDependency::query_data(cx, None, remote_fetch_timer).await?;
            tracing::debug!(points_len = full_data.len(), "calculating sum");
            let sum: DateValueString = sum::<DateValueInt>(&full_data, 0)?.into();
            let sum = sum.active_model(chart_id, Some(min_blockscout_block));
            insert_data_many(cx.db, vec![sum])
                .await
                .map_err(UpdateError::StatsDB)?;
            Ok(())
        }
    }
}
