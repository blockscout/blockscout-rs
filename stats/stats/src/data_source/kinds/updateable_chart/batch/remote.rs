//! Chart that is only directly dependant on
//! [`RemoteSource`](crate::data_source::kinds::remote::RemoteSource)
//! data source. It directly stores data obtained from the source
//! without any processing and/or transformations.

use std::marker::PhantomData;

use crate::{
    charts::{chart::chart_portrait, db_interaction::write::insert_data_many},
    data_source::kinds::remote::{RemoteSource, RemoteSourceWrapper},
    Chart, DateValue, UpdateError,
};

use super::{BatchDataSourceWrapper, BatchUpdateableChart};

/// See [module-level documentation](self) for details.
pub trait RemoteChart: Chart {
    type Dependency: RemoteSource;
}

pub type RemoteDataSourceWrapper<T> = BatchDataSourceWrapper<RemoteChartWrapper<T>>;

/// Wrapper struct used for avoiding implementation conflicts
///
/// See [module-level documentation](self) for details.
pub struct RemoteChartWrapper<T: RemoteChart>(PhantomData<T>);

#[portrait::fill(portrait::delegate(T))]
impl<T: RemoteChart + Chart> Chart for RemoteChartWrapper<T> {}

impl<T: RemoteChart> BatchUpdateableChart for RemoteChartWrapper<T> {
    type PrimaryDependency = RemoteSourceWrapper<T::Dependency>;
    type SecondaryDependencies = ();

    async fn batch_update_values_step_with(
        db: &sea_orm::prelude::DatabaseConnection,
        chart_id: i32,
        _update_time: chrono::DateTime<chrono::prelude::Utc>,
        min_blockscout_block: i64,
        primary_data: Vec<DateValue>,
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
