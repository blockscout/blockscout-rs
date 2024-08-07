use std::{collections::HashSet, future::Future, marker::Send, ops::Range};

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
    /// This data source relies on these sources for 'core' of its data.
    /// Tuple of data sources is also a data source.
    type MainDependencies: DataSource;
    /// Data sources that are used for computing various resolutions of data
    /// for now empty; until resolutions are introduced
    /// TODO: remove this ^ part when resolutions are finished
    type ResolutionDependencies: DataSource;
    /// Data that this source provides
    type Output: Send;

    /// Unique identifier of this data source that is used for synchronizing updates.
    ///
    /// Must be set to `Some` if the source stores some (local) data (i.e. `update_itself`
    /// does something) (e.g. [`local_db`](super::kinds::local_db))
    fn mutex_id() -> Option<String>;

    /// Initialize the data source and its dependencies.
    ///
    /// The intention is to leave default implementation and implement only
    /// [`DataSource::init_itself`].
    ///
    /// If the source was initialized before, keep old values. In other words,
    /// the method should be idempotent, as it is expected to be called on
    /// previously initialized sources.
    #[instrument(skip_all, level = tracing::Level::DEBUG, fields(source_mutex_id = Self::mutex_id()))]
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
    /// During normal operation calling this method directly is likely invalid.
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

    /// List MUTEX_ID's of itself (if any) and all of it's dependencies
    /// combined
    fn all_dependencies_mutex_ids() -> HashSet<String> {
        let mut ids = Self::MainDependencies::all_dependencies_mutex_ids();
        ids.extend(Self::ResolutionDependencies::all_dependencies_mutex_ids());
        if let Some(self_id) = Self::mutex_id() {
            let is_not_duplicate = ids.insert(self_id.clone());
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
    #[instrument(skip_all, level = tracing::Level::DEBUG, fields(source_mutex_id = Self::mutex_id()))]
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
    /// During normal operation calling this method directly is likely invalid, as to update
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
    /// Note that the returned data might have missing points for efficiency reasons.
    /// Meaning of the missing points is marked separately by `MissingDatePolicy`
    /// (for example, set in `ChartProperties`) or is just intrinsic to particular
    /// data source (i.e. it's not mentioned anywhere, should be checked
    /// case-by-case).
    ///
    /// **Does not perform an update!** If you need relevant data, you likely need
    /// to call [`DataSource::update_recursively`] beforehand.
    fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> impl Future<Output = Result<Self::Output, UpdateError>> + Send;
}

// Base case for recursive type
impl DataSource for () {
    type MainDependencies = ();
    type ResolutionDependencies = ();
    type Output = ();

    fn mutex_id() -> Option<String> {
        None
    }

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

    fn all_dependencies_mutex_ids() -> HashSet<String> {
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
        _range: Option<Range<DateTimeUtc>>,
        _remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        Ok(())
    }
}

macro_rules! impl_data_source_for_tuple {
    (( $($element_generic_name:ident),+ $(,)? )) => {
        impl< $($element_generic_name),+ > DataSource for ( $($element_generic_name),+ )
        where
            $(
                $element_generic_name: DataSource
            ),+
        {
            type MainDependencies = ();
            type ResolutionDependencies = ();
            type Output = ($(
                $element_generic_name::Output
            ),+);

            // only dependencies' ids matter
            fn mutex_id() -> Option<String> {
                None
            }

            async fn update_recursively(cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
                $(
                    $element_generic_name::update_recursively(cx).await?;
                )+
                Ok(())
            }

            async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
                // dependencies are called in `update_recursively`
                // the tuple itself does not need any init
                Ok(())
            }

            async fn query_data(
                cx: &UpdateContext<'_>,
                range: Option<Range<DateTimeUtc>>,
                remote_fetch_timer: &mut AggregateTimer,
            ) -> Result<Self::Output, UpdateError> {
                Ok((
                    $(
                        $element_generic_name::query_data(cx, range.clone(), remote_fetch_timer).await?
                    ),+
                ))
            }

            fn init_recursively<'a>(
                db: &'a DatabaseConnection,
                init_time: &'a chrono::DateTime<Utc>,
            ) -> BoxFuture<'a, Result<(), DbErr>> {
                async move {
                    $(
                        $element_generic_name::init_recursively(db, init_time).await?;
                    )+
                    Ok(())
                }
                .boxed()
            }

            async fn init_itself(
                _db: &DatabaseConnection,
                _init_time: &chrono::DateTime<Utc>,
            ) -> Result<(), DbErr> {
                // dependencies are called in `init_recursively`
                // the tuple itself does not need any init
                Ok(())
            }

            fn all_dependencies_mutex_ids() -> HashSet<String> {
                let mut ids = HashSet::new();
                $(
                    ids.extend($element_generic_name::all_dependencies_mutex_ids());
                )+
                ids
            }
        }
    };
}

// add more if needed
impl_data_source_for_tuple!((T1, T2));
impl_data_source_for_tuple!((T1, T2, T3));

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
                ContractsGrowth::mutex_id().unwrap(),
                NewContracts::mutex_id().unwrap(),
            ])
        );
        assert_eq!(
            NewContracts::all_dependencies_mutex_ids(),
            HashSet::from([NewContracts::mutex_id().unwrap(),])
        )
    }
}
