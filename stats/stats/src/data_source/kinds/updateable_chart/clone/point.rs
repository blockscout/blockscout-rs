//! Chart that directly stores data obtained from point source
//! without any processing and/or transformations.

use std::marker::PhantomData;

use crate::{
    charts::{chart::chart_portrait, db_interaction::write::insert_data_many},
    data_source::{
        kinds::updateable_chart::{UpdateableChart, UpdateableChartWrapper},
        DataSource,
    },
    Chart, DateValueString, Named, UpdateError,
};

/// See [module-level documentation](self) for details.
pub trait ClonePointChart: Chart {
    type Dependency: DataSource<Output = DateValueString>;

    fn batch_size() -> chrono::Duration {
        chrono::Duration::days(30)
    }
}

/// Wrapper to convert type implementing [`ClonePointChart`] to another that implements [`DataSource`]
pub type ClonePointChartWrapper<T> = UpdateableChartWrapper<ClonePointChartLocalWrapper<T>>;

/// Wrapper to get type implementing "parent" trait. Use [`ClonePointChartWrapper`] to get [`DataSource`]
pub struct ClonePointChartLocalWrapper<T: ClonePointChart>(PhantomData<T>);

impl<T: ClonePointChart + Named> Named for ClonePointChartLocalWrapper<T> {
    const NAME: &'static str = T::NAME;
}

#[portrait::fill(portrait::delegate(T))]
impl<T: ClonePointChart + Chart> Chart for ClonePointChartLocalWrapper<T> {}

impl<T: ClonePointChart> UpdateableChart for ClonePointChartLocalWrapper<T> {
    type PrimaryDependency = T::Dependency;
    type SecondaryDependencies = ();

    async fn update_values(
        cx: &crate::data_source::UpdateContext<'_>,
        chart_id: i32,
        _last_accurate_point: Option<DateValueString>,
        min_blockscout_block: i64,
        remote_fetch_timer: &mut blockscout_metrics_tools::AggregateTimer,
    ) -> Result<(), UpdateError> {
        // range doesn't make sense there; thus is not used
        let primary_data =
            Self::PrimaryDependency::query_data(cx, None, remote_fetch_timer).await?;
        let value = primary_data.active_model(chart_id, Some(min_blockscout_block));
        insert_data_many(cx.db, vec![value])
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}
