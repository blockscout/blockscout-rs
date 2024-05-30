//! Chart that directly stores data obtained from another source
//! without any processing and/or transformations.
//!
//! Useful, for example, for combination with adapter source.

use std::marker::PhantomData;

use crate::{
    charts::{chart::chart_portrait, db_interaction::write::insert_data_many},
    data_source::DataSource,
    Chart, DateValueString, Named, UpdateError,
};

use super::{BatchChart, BatchChartWrapper};

/// See [module-level documentation](self) for details.
pub trait CloneChart: Chart {
    type Dependency: DataSource<Output = Vec<DateValueString>>;
}

/// Wrapper to convert type implementing [`CloneChart`] to another that implements [`DataSource`]
pub type CloneChartWrapper<T> = BatchChartWrapper<CloneChartLocalWrapper<T>>;

/// Wrapper struct used for avoiding implementation conflicts
///
/// See [module-level documentation](self) for details.
pub struct CloneChartLocalWrapper<T: CloneChart>(PhantomData<T>);

impl<T: CloneChart + Named> Named for CloneChartLocalWrapper<T> {
    const NAME: &'static str = T::NAME;
}

#[portrait::fill(portrait::delegate(T))]
impl<T: CloneChart + Chart> Chart for CloneChartLocalWrapper<T> {}

impl<T: CloneChart> BatchChart for CloneChartLocalWrapper<T> {
    type PrimaryDependency = T::Dependency;
    type SecondaryDependencies = ();
    type Point = DateValueString;

    async fn batch_update_values_step_with(
        db: &sea_orm::prelude::DatabaseConnection,
        chart_id: i32,
        _update_time: chrono::DateTime<chrono::prelude::Utc>,
        min_blockscout_block: i64,
        primary_data: Vec<DateValueString>,
        _secondary_data: (),
    ) -> Result<usize, crate::UpdateError> {
        let found = primary_data.len();
        let values = primary_data
            .into_iter()
            .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(found)
    }
}
