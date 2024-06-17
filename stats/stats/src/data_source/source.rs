use std::{collections::HashSet, future::Future, marker::Send, ops::RangeInclusive};

use blockscout_metrics_tools::AggregateTimer;
use chrono::Utc;
use futures::{future::BoxFuture, FutureExt};
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};
use tracing::instrument;
use tynm::type_name;

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
    /// This data source relies on these sources for 'core' of its data
    type MainDependencies: DataSource;
    /// Data sources that are used for computing various resolutions of data
    /// for now empty; until resolutions are introduced
    /// TODO: remove this ^ part when resolutions are finished
    type ResolutionDependencies: DataSource;
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
    #[instrument(skip_all, level = tracing::Level::DEBUG, fields(source_mutex_id = Self::MUTEX_ID))]
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
            tracing::debug!("recursively initializing main dependencies");
            Self::MainDependencies::init_recursively(db, init_time).await?;
            tracing::debug!("recursively initializing resolution dependencies");
            Self::ResolutionDependencies::init_recursively(db, init_time).await?;
            tracing::debug!("initializing itself");
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
    ) -> impl Future<Output = Result<(), DbErr>> + Send;

    fn all_dependencies_mutex_ids() -> HashSet<&'static str> {
        let mut ids = Self::MainDependencies::all_dependencies_mutex_ids();
        ids.extend(Self::ResolutionDependencies::all_dependencies_mutex_ids());
        if let Some(self_id) = Self::MUTEX_ID {
            let is_not_duplicate = ids.insert(self_id);
            // Type system shouldn't allow same type to be present in the dependencies,
            // so it is a duplicate name. Fail fast to detect the problem early and
            // not cause any disruptions in database.
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
    // aboba
    #[instrument(skip_all, level = tracing::Level::DEBUG, fields(source_mutex_id = Self::MUTEX_ID))]
    fn update_recursively(
        cx: &UpdateContext<'_>,
    ) -> impl Future<Output = Result<(), UpdateError>> + Send {
        async move {
            // Couldn't figure out how to control level per-argument basis (in instrumentation)
            // so this event is used insted, since the name is usually quite verbose
            // + the type is still recursive and stacking them is kinda overwhelming.

            // to check all logs within the type one can find the event with matching type
            // and use `source_mutex_id` of the event. It should be suitable for general case.
            tracing::debug!("updating {}", type_name::<Self>());
            tracing::debug!("recursively updating primary dependency");
            Self::MainDependencies::update_recursively(cx).await?;
            tracing::debug!("recursively updating secondary dependencies");
            Self::ResolutionDependencies::update_recursively(cx).await?;
            tracing::debug!("updating itself");
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
    ) -> impl Future<Output = Result<(), UpdateError>> + Send;

    /// Retrieve chart data.
    /// If `range` is `Some`, should return data within the range. Otherwise - all data.
    ///
    /// Note that the data might have missing points for efficiency reasons.
    ///
    /// **Does not perform an update!** If you need relevant data, you likely need
    /// to call [`DataSource::update_recursively`] beforehand.
    fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<RangeInclusive<DateTimeUtc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> impl Future<Output = Result<Self::Output, UpdateError>> + Send;
}

// Base case for recursive type
impl DataSource for () {
    type MainDependencies = ();
    type ResolutionDependencies = ();
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
        _range: Option<RangeInclusive<DateTimeUtc>>,
        _remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        Ok(())
    }
}

// todo: impl for tuples (i.e. up to 4-5) with macro
// and without violating main/resolution deps difference
impl<T1, T2> DataSource for (T1, T2)
where
    T1: DataSource,
    T2: DataSource,
{
    type MainDependencies = T1;
    type ResolutionDependencies = T2;
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
        range: Option<RangeInclusive<DateTimeUtc>>,
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

    use crate::data_source::tests::{ContractsGrowth, NewContracts};

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
