//! Remote source that returns a single point.
//! Basically a coutner.

use std::{marker::PhantomData, ops::RangeInclusive};

use blockscout_metrics_tools::AggregateTimer;
use sea_orm::{prelude::DateTimeUtc, FromQueryResult, Statement};

use crate::{
    charts::chart::Point,
    data_source::{source::DataSource, types::UpdateContext},
    Named, UpdateError,
};

/// See [module-level documentation](self) for details.
pub trait RemotePointSource {
    /// Type of point to get from the query.
    /// `DateValueString` can be used to avoid parsing the values,
    /// but `DateValueDecimal` or other types can be useful sometimes.
    type Point: Point;

    fn get_query() -> Statement;

    fn query_data(
        cx: &UpdateContext<'_>,
    ) -> impl std::future::Future<Output = Result<Self::Point, UpdateError>> + std::marker::Send
    {
        async move {
            let query = Self::get_query();
            let data = Self::Point::find_by_statement(query)
                .one(cx.blockscout)
                .await
                .map_err(UpdateError::BlockscoutDB)?
                .ok_or_else(|| UpdateError::Internal("query returned nothing".into()))?;
            Ok(data)
        }
    }
}

/// Wrapper to convert type implementing [`RemoteSource`] to another that implements [`DataSource`]
pub struct RemotePointSourceWrapper<T: RemotePointSource>(PhantomData<T>);

impl<T: RemotePointSource + Named> Named for RemotePointSourceWrapper<T> {
    const NAME: &'static str = T::NAME;
}

impl<T: RemotePointSource> DataSource for RemotePointSourceWrapper<T> {
    type PrimaryDependency = ();
    type SecondaryDependencies = ();
    type Output = T::Point;
    // No local state => no race conditions expected
    const MUTEX_ID: Option<&'static str> = None;

    async fn init_itself(
        _db: &::sea_orm::DatabaseConnection,
        _init_time: &::chrono::DateTime<chrono::Utc>,
    ) -> Result<(), ::sea_orm::DbErr> {
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: Option<RangeInclusive<DateTimeUtc>>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<<Self as DataSource>::Output, UpdateError> {
        let _interval = remote_fetch_timer.start_interval();
        T::query_data(cx).await
    }

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        Ok(())
    }
}
