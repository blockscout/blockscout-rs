//! Chart that directly stores data obtained from another source
//! without any processing and/or transformations.
//!
//! Useful, for example, for combination with adapter source.

pub mod point;

use crate::{data_source::DataSource, Chart, DateValueString};

use super::batch::BatchChartWrapper;

/// See [module-level documentation](self) for details.
pub trait CloneChart: Chart {
    type Dependency: DataSource<Output = Vec<DateValueString>>;

    fn batch_size() -> chrono::Duration {
        chrono::Duration::days(30)
    }
}

/// Wrapper to convert type implementing [`CloneChart`] to another that implements [`DataSource`]
pub type CloneChartWrapper<T> = BatchChartWrapper<_inner::CloneChartLocalWrapper<T>>;

mod _inner {
    use std::marker::PhantomData;

    use crate::{
        charts::{chart::chart_portrait, db_interaction::write::insert_data_many},
        data_source::kinds::updateable_chart::batch::BatchChart,
        Chart, DateValueString, Named, UpdateError,
    };

    use super::CloneChart;
    /// Wrapper to get type implementing "parent" trait. Use [`super::CloneChartWrapper`] to get [`super::DataSource`]
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
            // note: right away cloning another chart will not result in exact copy,
            // because if the other chart is `FillPrevious`, then omitted starting point
            // within the range is set to the last known before the range
            // i.e. some duplicate points might get added.
            //
            // however, it should not be a problem, since semantics remain the same +
            // cloning already stored chart is counter-productive/not effective.
            let values = primary_data
                .into_iter()
                .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
            insert_data_many(db, values)
                .await
                .map_err(UpdateError::StatsDB)?;
            Ok(found)
        }

        fn batch_len() -> chrono::Duration {
            T::batch_size()
        }
    }
}
