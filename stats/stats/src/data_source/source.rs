use std::ops::RangeInclusive;

use blockscout_metrics_tools::AggregateTimer;
use chrono::{NaiveDate, Utc};
use futures::{future::BoxFuture, FutureExt};
use sea_orm::{DatabaseConnection, DbErr};

use crate::UpdateError;

use super::types::{UpdateContext, UpdateParameters};

/// Thing that can provide data from local storage.
///
/// See [`update`](`LocalDataSource::update`) and [`get_local`](`LocalDataSource::get_local`)
/// for functionality details.
///
/// Usually it's a chart that can:
///     - depend only on external data (i.e. independent from local data)
///     - depend on data from other charts
///
/// Also it can be a remote data source.
pub trait DataSource {
    type PrimaryDependency: DataSource;
    type SecondaryDependencies: DataSource;
    type Output;

    /// Initialize the data source and its dependencies in local database.
    ///
    /// If the source was initialized before, keep old values.
    fn init_all_locally<'a>(
        db: &'a DatabaseConnection,
        init_time: &'a chrono::DateTime<Utc>,
    ) -> BoxFuture<'a, Result<(), DbErr>> {
        async move {
            Self::PrimaryDependency::init_all_locally(db, init_time).await?;
            Self::SecondaryDependencies::init_all_locally(db, init_time).await?;
            Self::init_itself(db, init_time).await
        }
        .boxed()
        // had to juggle with boxed futures
        // because of recursive async calls (:
        // :)
    }

    /// Initialize only this source. This fn is intended to be implemented
    /// for regular types
    fn init_itself(
        db: &DatabaseConnection,
        init_time: &chrono::DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<(), DbErr>> + Send;

    /// Update source data (values + metadata), if necessary.
    async fn update_from_remote(
        cx: &UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError>;

    /// Retrieve chart data for dates in `range`.
    ///
    /// **Does not perform an update!** If you need relevant data, you likely need
    /// to call [`DataSource::update_from_remote`] beforehand.
    async fn query_data(
        cx: &UpdateContext<UpdateParameters<'_>>,
        range: RangeInclusive<NaiveDate>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError>;
}

// Base case for recursive type
impl DataSource for () {
    type PrimaryDependency = ();
    type SecondaryDependencies = ();
    type Output = ();

    fn init_all_locally<'a>(
        _db: &'a DatabaseConnection,
        _init_time: &'a chrono::DateTime<Utc>,
    ) -> BoxFuture<'a, Result<(), DbErr>> {
        // stop recursion
        async { Ok(()) }.boxed()
    }

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr> {
        // todo: unimplemented where applicable?
        Ok(())
    }

    async fn update_from_remote(
        _cx: &UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError> {
        // stop recursion
        Ok(())
    }
    async fn query_data(
        _cx: &UpdateContext<UpdateParameters<'_>>,
        _range: RangeInclusive<NaiveDate>,
        _remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        Ok(())
    }
}

impl<T1, T2> DataSource for (T1, T2)
where
    T1: DataSource,
    T2: DataSource,
{
    type PrimaryDependency = T1;
    type SecondaryDependencies = T2;
    type Output = (T1::Output, T2::Output);

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr> {
        // dependencies are called in `init_all_locally`
        // the tuple itself does not need any init
        Ok(())
    }

    /// Update source data (values + metadata), if necessary.
    async fn update_from_remote(
        cx: &UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError> {
        Self::PrimaryDependency::update_from_remote(cx).await?;
        Self::SecondaryDependencies::update_from_remote(cx).await?;
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<UpdateParameters<'_>>,
        range: RangeInclusive<NaiveDate>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        Ok((
            T1::query_data(cx, range.clone(), remote_fetch_timer).await?,
            T2::query_data(cx, range, remote_fetch_timer).await?,
        ))
    }
}
