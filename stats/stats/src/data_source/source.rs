use std::{collections::HashSet, ops::RangeInclusive};

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

    /// Unique identifier of this data source that is used for synchronizing updates.
    ///
    /// Must be set to `Some` if the source stores some local data (i.e. `update_from_remote`
    /// does something)
    const MUTEX_ID: Option<&'static str>;

    /// Initialize the data source and its dependencies in local database.
    ///
    /// The intention is to leave default implementation and implement only
    /// [`DataSource::init_itself`].
    ///
    /// If the source was initialized before, keep old values. In other words,
    /// the method should be idempotent, as it is expected to be called on
    /// previously initialized sources.
    fn init_all_locally<'a>(
        db: &'a DatabaseConnection,
        init_time: &'a chrono::DateTime<Utc>,
    ) -> BoxFuture<'a, Result<(), DbErr>> {
        async move {
            // It is possible to do it concurrently, but it requires mutexes on
            // each chart.
            // We don't want any race conditions that could happen when some source D
            // is referenced through two different charts.
            //     C → D
            //   ↗   ↗
            // A → B
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
    /// by types, as recursive logic of [`DataSource::init_all_locally`] is
    /// not expected to change.
    ///
    /// Should be idempotent.
    fn init_itself(
        db: &DatabaseConnection,
        init_time: &chrono::DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<(), DbErr>> + Send;

    fn all_dependencies_mutex_ids() -> HashSet<&'static str> {
        let mut ids = Self::PrimaryDependency::all_dependencies_mutex_ids();
        ids.extend(Self::SecondaryDependencies::all_dependencies_mutex_ids().into_iter());
        if let Some(self_id) = Self::MUTEX_ID {
            let is_not_duplicate = ids.insert(self_id);
            // Type system shouldn't allow same type to be present in the dependencies,
            // so it is a duplicate name
            assert!(
                is_not_duplicate,
                "Data sources `MUTEX_ID`s must be unique. ID '{self_id}' is duplicate",
            );
        }
        ids
    }

    /// Update source data (values + metadata).
    ///
    /// Should be idempontent with regards to `current_time` (in `cx`).
    /// It is a normal behaviour to call this method within single update.
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
    const MUTEX_ID: Option<&'static str> = None;

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

    fn all_dependencies_mutex_ids() -> HashSet<&'static str> {
        HashSet::new()
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

    // only dependencies' ids matter
    const MUTEX_ID: Option<&'static str> = None;

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr> {
        // dependencies are called in `init_all_locally`
        // the tuple itself does not need any init
        Ok(())
    }

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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::data_source::example::{ContractsGrowthChartSource, NewContractsChartSource};

    use super::DataSource;

    #[test]
    fn dependencies_listed_correctly() {
        assert_eq!(
            ContractsGrowthChartSource::all_dependencies_mutex_ids(),
            HashSet::from([
                ContractsGrowthChartSource::MUTEX_ID.unwrap(),
                NewContractsChartSource::MUTEX_ID.unwrap(),
            ])
        );
        assert_eq!(
            NewContractsChartSource::all_dependencies_mutex_ids(),
            HashSet::from([NewContractsChartSource::MUTEX_ID.unwrap(),])
        )
    }
}
