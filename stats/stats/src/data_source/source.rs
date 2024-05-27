use std::{collections::HashSet, ops::RangeInclusive};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{NaiveDate, Utc};
use futures::{future::BoxFuture, FutureExt};
use sea_orm::{DatabaseConnection, DbErr};

use crate::UpdateError;

use super::types::UpdateContext;

/// Thing that can provide data.
///
/// Methods expected to be called:
/// - [`DataSource::init_recursively`]
/// - [`DataSource::update_recursively`]
/// - [`DataSource::all_dependencies_mutex_ids`]
/// - [`DataSource::query_data`]
///
/// Methods expected to be implemented:
/// - [`DataSource::init_itself`]
/// - [`DataSource::update_itself`]
/// - [`DataSource::query_data`]
///
/// See [`super::kinds`] for less general cases.
pub trait DataSource {
    /// Dependency that is considered "main" for this data source.
    ///
    /// In practice, primary/secondary are not distinguished. The
    /// naming is just for convenience/readability/indication.
    type PrimaryDependency: DataSource;
    /// Dependency that is considered less important than [`DataSource::PrimaryDependency`]
    /// for this data source.
    ///
    /// In practice, primary/secondary are not distinguished. The
    /// naming is just for convenience/readability/indication.
    type SecondaryDependencies: DataSource;
    /// Data that this source can provide
    type Output: Send;

    /// Unique identifier of this data source that is used for synchronizing updates.
    ///
    /// Must be set to `Some` if the source stores some (local) data (i.e. `update_itself`
    /// does something)
    const MUTEX_ID: Option<&'static str>;

    /// Initialize the data source and its dependencies.
    ///
    /// The intention is to leave default implementation and implement only
    /// [`DataSource::init_itself`].
    ///
    /// If the source was initialized before, keep old values. In other words,
    /// the method should be idempotent, as it is expected to be called on
    /// previously initialized sources.
    fn init_recursively<'a>(
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
            // todo: log deps init??
            // tracing::info!(
            //     chart_name = Self::NAME,
            //     parent_chart_name = P::NAME,
            //     "init parent"
            // );
            Self::PrimaryDependency::init_recursively(db, init_time).await?;
            Self::SecondaryDependencies::init_recursively(db, init_time).await?;
            Self::init_itself(db, init_time).await
        }
        .boxed()
        // had to juggle with boxed futures
        // because of recursive async calls (:
        // :)
    }

    /// **DO NOT CALL DIRECTLY** unless you don't want dependencies to be initialized.
    /// During normal operation this method is likely invalid.
    ///
    /// This fn is intended to be implemented
    /// by types, as recursive logic of [`DataSource::init_recursively`] is
    /// not expected to change.
    ///
    /// Should be idempotent.
    ///
    /// ## Description
    /// Initialize only this source.
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

    /// Update dependencies' and this source's data (values + metadata).
    ///
    /// Should be idempontent with regards to `current_time` (in `cx`).
    /// It is a normal behaviour to call this method multiple times
    /// within single update.
    fn update_recursively(
        cx: &UpdateContext<'_>,
    ) -> impl std::future::Future<Output = Result<(), UpdateError>> + std::marker::Send {
        async move {
            // todo: log deps updated
            // tracing::info!(
            //     chart_name = Self::NAME,
            //     parent_chart_name = P::NAME,
            //     "updating parent"
            // );
            Self::PrimaryDependency::update_recursively(cx).await?;
            Self::SecondaryDependencies::update_recursively(cx).await?;
            Self::update_itself(cx).await
        }
    }

    /// **DO NOT CALL DIRECTLY** unless you don't want dependencies to be updated.
    /// During normal operation this method is likely invalid, as to update
    /// this source, dependencies have to be in a relevant state.
    ///
    /// This fn is intended to be implemented
    /// by types, as recursive logic of [`DataSource::update_recursively`] is
    /// not expected to change.
    ///
    /// Should be idempotent.
    ///
    /// ## Description
    /// Update only thise data source's data (values + metadat)
    fn update_itself(
        cx: &UpdateContext<'_>,
    ) -> impl std::future::Future<Output = Result<(), UpdateError>> + std::marker::Send;

    /// Retrieve chart data for dates in `range`.
    ///
    /// **Does not perform an update!** If you need relevant data, you likely need
    /// to call [`DataSource::update_recursively`] beforehand.
    fn query_data(
        cx: &UpdateContext<'_>,
        range: RangeInclusive<NaiveDate>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> impl std::future::Future<Output = Result<Self::Output, UpdateError>> + std::marker::Send;
}

// Base case for recursive type
impl DataSource for () {
    type PrimaryDependency = ();
    type SecondaryDependencies = ();
    type Output = ();
    const MUTEX_ID: Option<&'static str> = None;

    fn init_recursively<'a>(
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
        unreachable!("not called by `init_recursively` and must not be called by anything else")
    }

    fn all_dependencies_mutex_ids() -> HashSet<&'static str> {
        HashSet::new()
    }

    async fn update_recursively(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        // stop recursion
        Ok(())
    }

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        unreachable!("not called by `update_recursively` and must not be called by anything else")
    }

    async fn query_data(
        _cx: &UpdateContext<'_>,
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
        // dependencies are called in `init_recursively`
        // the tuple itself does not need any init
        Ok(())
    }

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
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

    use crate::data_source::example::{ContractsGrowth, NewContracts};

    use super::DataSource;

    #[test]
    fn dependencies_listed_correctly() {
        assert_eq!(
            ContractsGrowth::all_dependencies_mutex_ids(),
            HashSet::from([
                ContractsGrowth::MUTEX_ID.unwrap(),
                NewContracts::MUTEX_ID.unwrap(),
            ])
        );
        assert_eq!(
            NewContracts::all_dependencies_mutex_ids(),
            HashSet::from([NewContracts::MUTEX_ID.unwrap(),])
        )
    }
}
