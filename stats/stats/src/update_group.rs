//! Chart update groups.
//!
//! ## Justification
//!
//! Reasons to have update groups are listed in [data source module docs (Usage)](crate::data_source)
//! and `construct_update_group` macro docs.
//!
//! ## Usage
//!
//! You can also check [`data_source` documentation](crate::data_source), tests there, or
//! the actual usage in `stats-server` crate (`runtime_setup.rs`).
//!
//! Note: instructions provided here are for manual/direct usage of the update groups.
//! If you're not doing somerhing exceptional see instructions in `runtime_setup.rs`
//! that has everything already set up.
//!
//!
//! 1. Create multiple connected charts
//!    (e.g.
//!    [`DirectVecLocalDbChartSource`](crate::data_source::kinds::local_db::DirectVecLocalDbChartSource)
//!    or
//!    [`DailyCumulativeLocalDbChartSource`](crate::data_source::kinds::local_db::DailyCumulativeLocalDbChartSource)
//!    ).
//! 2. Construct simple (non-sync) update groups via `construct_update_group` macro (usually done in `../update_groups.rs`)
//! 3. Create mutexes (1-1 for each chart) (in general automatically done in `stats-server`, see instructions there
//!    in `runtime_setup.rs`)
//! 4. Create synchronous versions of groups with [`SyncUpdateGroup::new`]
//!    (in general automatically done in `stats-server`, see instructions there)
//!

use std::{
    collections::{BTreeMap, HashSet},
    marker::{Send, Sync},
    sync::Arc,
    vec::Vec,
};

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use itertools::Itertools;
use sea_orm::{DatabaseConnection, DbErr};
use thiserror::Error;
use tokio::sync::{Mutex, MutexGuard};

use crate::{
    ChartError,
    charts::{ChartObject, chart_properties_portrait::imports::ChartKey},
    data_source::UpdateParameters,
    indexing_status::IndexingStatus,
};

#[derive(Error, Debug, PartialEq)]
#[error("Could not initialize update group: mutexes for {missing_mutexes:?} were not provided")]
pub struct InitializationError {
    pub missing_mutexes: Vec<String>,
}

// todo: use `trait-variant` once updated, probably
// only `async_trait` currently allows making trait objects
// https://github.com/rust-lang/impl-trait-utils/issues/34

/// Directed Acyclic Connected Graph of charts.
///
/// Generally (when there are >1 update groups), [`SyncUpdateGroup`]
/// should be used instead. It provides synchronization mechanism
/// that prevents data races between the groups.
#[async_trait]
pub trait UpdateGroup: core::fmt::Debug {
    // &self's is only to make fns dispatchable (and trait to be object-safe)

    /// Group name (usually equal to type name for simplicity)
    fn name(&self) -> String;
    /// List chart properties - members of the group.
    fn list_charts(&self) -> Vec<ChartObject>;
    /// Indexing status that is needed to update all enabled members of the group
    fn dependency_indexing_status_requirement(
        &self,
        enabled_charts: &HashSet<ChartKey>,
    ) -> IndexingStatus;
    /// List mutex ids of group members + their dependencies.
    /// Dependencies participate in updates, thus access to them needs to be
    /// synchronized as well.
    fn list_dependency_mutex_ids(&self) -> HashSet<String>;
    /// List mutex ids of particular group member dependencies (including the member itself).
    ///
    /// `None` if `chart_id`` is not a member.
    fn dependency_mutex_ids_of(&self, chart_id: &ChartKey) -> Option<HashSet<String>>;
    /// List chart keys of a particular group member dependencies (including the member itself).
    ///
    /// `None` if `chart_id`` is not a member.
    fn dependency_keys_of(&self, chart_id: &ChartKey) -> Option<HashSet<ChartKey>>;
    /// Create/init enabled charts with their dependencies (in DB) recursively.
    /// Idempotent, does nothing if the charts were previously initialized.
    ///
    /// `creation_time_override` is for overriding creation time (only works for not
    /// initialized charts). Helpful for testing.
    async fn create_charts(
        &self,
        db: &DatabaseConnection,
        creation_time_override: Option<chrono::DateTime<Utc>>,
        enabled_charts: &HashSet<ChartKey>,
    ) -> Result<(), DbErr>;
    /// Update enabled charts and their dependencies in one go.
    ///
    /// Recursively updates dependencies first.
    async fn update_charts<'a>(
        &self,
        params: UpdateParameters<'a>,
        enabled_charts: &HashSet<ChartKey>,
    ) -> Result<(), ChartError>;
    /// Reset the enabled charts so that they reupdate all their
    /// data starting from `from`
    ///
    /// Recursively applies the same to the dependencies first
    async fn set_next_update_from(
        &self,
        db: &DatabaseConnection,
        from: NaiveDate,
        enabled_charts: &HashSet<ChartKey>,
    ) -> Result<(), ChartError>;
}

/// Construct update group that implemants [`UpdateGroup`]. The main purpose of the
/// group is to update its members together.
///
/// All membere must implement [`trait@crate::ChartProperties`] and [`crate::data_source::DataSource`].
///
/// The behaviour is the following:
/// 1. when `create` or `update` is triggered, each member's correspinding method is triggered
/// 2. the method recursively updates/creates all of the member's dependencies
///
/// In addition, to update synchronization, the resulting group provides object-safe interface
/// for interacting with its members (charts). In particular, the charts themselves
/// intentionally do not have `self` parameter, which is present in the resulting group.
///
/// ## Usage
///
/// Basic usage:
/// ```rust
/// # use stats::data_source::kinds::{
/// # };
/// # use stats::{ChartProperties, Named, construct_update_group, types::timespans::DateValue, ChartError};
/// # use stats::data_source::{
/// #     kinds::{
/// #         local_db::{DirectVecLocalDbChartSource, parameters::update::batching::parameters::Batch30Days},
/// #         remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
/// #         data_manipulation::map::{MapToString, StripExt},
/// #     },
/// #     types::{UpdateContext, UpdateParameters},
/// # };
/// # use stats::lines::NewBlocks;
/// # use chrono::NaiveDate;
/// # use entity::sea_orm_active_enums::ChartType;
/// # use std::ops::RangeInclusive;
/// # use sea_orm::Statement;
/// #
/// # struct DummyChartProperties;
/// #
/// # impl Named for DummyChartProperties {
/// #     fn name() -> String {
/// #         "dummyChart".into()
/// #     }
/// # }
/// # impl ChartProperties for DummyChartProperties {
/// #     type Resolution = NaiveDate;
/// #     fn chart_type() -> ChartType {
/// #         ChartType::Line
/// #     }
/// # }
/// #
/// # type DummyChart = DirectVecLocalDbChartSource<StripExt<NewBlocks>, Batch30Days, DummyChartProperties>;
///
/// construct_update_group!(ExampleUpdateGroup {
///     charts: [DummyChart],
/// });
/// ```
///
/// Since `update` and `create` are performed recursively it makes sense to include
/// all ancestors of members (i.e. dependencies, dependencies of them, etc.) into the group.
/// Otherwise the following may occur:
///
/// Let's say we have chart `A` that depends on `B`. We make an update group with only `A`.
///
/// > A ⇨ B
///
/// See possible configurations of enabled charts:
///
/// - Both `A` and `B` are enabled:\
/// > **A** ➡ **B**
/// > Group triggers `A`, which triggers `B`. Everything is fine.
///
/// - `A` is on, `B` is off:\
/// > **A** ➡ **B**\
/// > Group triggers `A`, which triggers `B`. Everything is fine.
///
/// - `A` is off, `B` is on:\
/// > A ⇨ B\
/// > Group only contains `A`, which means nothing is triggered. Quite counter-intuitive
/// > (`B` is not triggered even though it's enabled).
///
/// - `A` is off, `B` is off:\
/// > A ⇨ B\
/// > Nothing happens, as expected.
///
/// Therefore, to make working with updates easier, it is highly recommended to include all
/// dependencies into the group. Later this check might be included into the macro.
///
/// Similarly, it makes sense to include all dependants (unless they have some other heavy dependencies),
/// since it should be very lightweight to update them together.
///
/// ## Reasoning for macro
///
/// Since group member types are different (and trait impls have different associated types),
/// we can't use homogeneous collections like `Vec`.
///
/// Therefore, the macro helps to avoid boilerplate when defining the update groups.
///
/// ## Example
///
/// ```rust
/// # use stats::{
/// #     QueryAllBlockTimestampRange, construct_update_group,
/// #     types::timespans::DateValue, ChartProperties, Named, ChartError,
/// #     ChartKey,
/// # };
/// # use stats::data_source::{
/// #     kinds::{
/// #         local_db::{DirectVecLocalDbChartSource, parameters::update::batching::parameters::Batch30Days},
/// #         remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
/// #     },
/// #     types::{UpdateContext, UpdateParameters, IndexerMigrations},
/// # };
/// # use chrono::{NaiveDate, DateTime, Utc};
/// # use entity::sea_orm_active_enums::ChartType;
/// # use std::{ops::Range, collections::HashSet};
/// # use sea_orm::Statement;
///
/// struct DummyRemoteStatement;
///
/// impl StatementFromRange for DummyRemoteStatement {
///     fn get_statement(range: Option<Range<DateTime<Utc>>>, _: &IndexerMigrations, _: &HashSet<ChartKey>) -> Statement {
///         todo!()
///     }
/// }
///
/// type DummyRemote = RemoteDatabaseSource<PullAllWithAndSort<DummyRemoteStatement, NaiveDate, String, QueryAllBlockTimestampRange>>;
///
/// struct DummyChartProperties;
///
/// impl Named for DummyChartProperties {
///     fn name() -> String {
///         "dummyChart".into()
///     }
/// }
/// impl ChartProperties for DummyChartProperties {
///     type Resolution = NaiveDate;
///     fn chart_type() -> ChartType {
///         ChartType::Line
///     }
/// }
///
/// type DummyChart = DirectVecLocalDbChartSource<DummyRemote, Batch30Days, DummyChartProperties>;
///
/// construct_update_group!(ExampleUpdateGroup {
///     charts: [DummyChart],
/// });
/// ```
///
#[macro_export]
macro_rules! construct_update_group {
    ($group_name:ident {
        charts: [
            $($member:path),+
            $(,)?
        ] $(,)?
    }) => {
        #[derive(::core::fmt::Debug)]
        pub struct $group_name;

        #[::async_trait::async_trait]
        impl $crate::update_group::UpdateGroup for $group_name
        {
            fn name(&self) -> ::std::string::String {
                stringify!($group_name).into()
            }

            fn list_charts(&self) -> ::std::vec::Vec<$crate::ChartObject> {
                std::vec![
                    $(
                        $crate::ChartObject::construct_from_chart::<$member>(
                            <$member as $crate::query_dispatch::QuerySerialized>::new_for_dynamic_dispatch()
                        ),
                    )*
                ]
            }

            fn dependency_indexing_status_requirement(
                &self,
                enabled_charts: &::std::collections::HashSet<$crate::ChartKey>,
            ) -> $crate::indexing_status::IndexingStatus {
                use $crate::indexing_status::IndexingStatusTrait;

                $crate::indexing_status::IndexingStatus::most_restrictive_from(
                    [
                        $(
                            (
                                <$member as $crate::ChartProperties>::key(),
                                <$member as $crate::data_source::DataSource>::indexing_status_requirement_recursive()
                            ),
                        )*
                    ]
                    .into_iter()
                    .filter(|(k, _)| enabled_charts.contains(k))
                    .map(|(_, req)| req),
                )

            }

            fn list_dependency_mutex_ids(&self) -> ::std::collections::HashSet<::std::string::String> {
                let mut ids = ::std::collections::HashSet::new();
                $(
                    ids.extend(
                        <$member as $crate::data_source::DataSource>::all_dependencies_mutex_ids()
                            .into_iter()
                    );
                )*
                ids
            }

            fn dependency_mutex_ids_of(
                &self,
                chart_id: &$crate::ChartKey
            ) -> Option<::std::collections::HashSet<::std::string::String>> {
                $(
                    if chart_id == &<$member as $crate::ChartProperties>::key() {
                        return Some(
                            <$member as $crate::data_source::DataSource>::all_dependencies_mutex_ids()
                        );
                    }
                )*
                return None;
            }

            fn dependency_keys_of(
                &self,
                chart_id: &$crate::ChartKey)
            -> Option<::std::collections::HashSet<$crate::ChartKey>> {
                $(
                    if chart_id == &<$member as $crate::ChartProperties>::key() {
                        return Some(
                            <$member as $crate::data_source::DataSource>::all_dependencies_chart_keys()
                        );
                    }
                )*
                return None;
            }

            async fn create_charts(
                &self,
                #[allow(unused)]
                db: &sea_orm::DatabaseConnection,
                #[allow(unused)]
                creation_time_override: ::std::option::Option<
                    ::chrono::DateTime<::chrono::Utc>
                >,
                #[allow(unused)]
                enabled_charts: &::std::collections::HashSet<$crate::ChartKey>,
            ) -> Result<(), sea_orm::DbErr> {
                let current_time = creation_time_override.unwrap_or_else(|| ::chrono::Utc::now());
                $(
                    if enabled_charts.contains(&<$member as $crate::ChartProperties>::key()) {
                        <$member as $crate::data_source::DataSource>::init_recursively(
                            db, &current_time
                        ).await?;
                    }
                )*
                Ok(())
            }

            // updates are expected to be unique by group name & update time; this instrumentation
            // should allow to single out one update process in logs
            #[::tracing::instrument(
                skip_all,
                fields(update_group=self.name(), update_time),
                level = tracing::Level::INFO
            )]
            async fn update_charts<'a>(
                &self,
                params: $crate::data_source::UpdateParameters<'a>,
                #[allow(unused)]
                enabled_charts: &::std::collections::HashSet<$crate::ChartKey>,
            ) -> Result<(), $crate::ChartError> {
                let cx = $crate::data_source::UpdateContext::from_params_now_or_override(params);
                ::tracing::Span::current().record("update_time", ::std::format!("{}",&cx.time));
                $(
                    if enabled_charts.contains(&<$member as $crate::ChartProperties>::key()) {
                        <$member as $crate::data_source::DataSource>::update_recursively(&cx).await?;
                    }
                )*
                Ok(())
            }

            // pretty straightforward, no need for instrumentation
            async fn set_next_update_from(
                &self,
                #[allow(unused)]
                db: &sea_orm::DatabaseConnection,
                from: chrono::NaiveDate,
                #[allow(unused)]
                enabled_charts: &::std::collections::HashSet<$crate::ChartKey>,
            ) -> Result<(), $crate::ChartError> {
                $(
                    if enabled_charts.contains(&<$member as $crate::ChartProperties>::key()) {
                        <$member as $crate::data_source::DataSource>::set_next_update_from_recursively(db, from.clone()).await?;
                    }
                )*
                Ok(())
            }
        }

    };
}

pub type ArcUpdateGroup = Arc<dyn UpdateGroup + Send + Sync + 'static>;

/// Synchronized update group. Wrapper around [`UpdateGroup`] with
/// synchronization mechanism.
///
/// ## Deadlock-free
/// Deadlock-free, as long as only `SyncUpdateGroup`s are used
/// and mapping name-mutex is consistent between the groups
/// (i.e. single mutex is shared for the chart with the
/// same name (= same chart, since names must be unique)).
///
/// This is achieved using deadlock ordering.
/// For more info see [link, section "lock ordering"](https://www.cs.cornell.edu/courses/cs4410/2017su/lectures/lec09-deadlock.html)
///
/// The order is a lexicographical order of chart (data source) mutex IDs
#[derive(Debug, Clone)]
pub struct SyncUpdateGroup {
    /// Mutexes. Acquired in lexicographical order (=order within `BTreeMap`)
    dependencies_mutexes: BTreeMap<String, Arc<Mutex<()>>>,
    inner: ArcUpdateGroup,
}

impl SyncUpdateGroup {
    /// `all_chart_mutexes` must contain mutexes for all members of the group + their dependencies
    /// (will return error otherwise).
    ///
    /// These mutexes must be shared across all groups in order for synchronization to work.
    pub fn new(
        all_chart_mutexes: &BTreeMap<String, Arc<Mutex<()>>>,
        inner: ArcUpdateGroup,
    ) -> Result<Self, InitializationError>
    where
        Self: Sized,
    {
        let dependencies: HashSet<String> = inner.list_dependency_mutex_ids();
        let received_charts: HashSet<String> = all_chart_mutexes.keys().cloned().collect();
        let missing_mutexes = dependencies
            .difference(&received_charts)
            .map(|s| (*s).to_owned())
            .collect_vec();

        let mut dependencies_mutexes = BTreeMap::new();

        for dependency_name in dependencies {
            let mutex = all_chart_mutexes
                .get(&dependency_name)
                .ok_or(InitializationError {
                    missing_mutexes: missing_mutexes.clone(),
                })?;
            dependencies_mutexes.insert(dependency_name.to_owned(), mutex.clone());
        }
        Ok(Self {
            dependencies_mutexes,
            inner,
        })
    }

    /// See [`UpdateGroup::name`]
    pub fn name(&self) -> String {
        self.inner.name()
    }

    /// See [`UpdateGroup::list_charts`]
    pub fn list_charts(&self) -> Vec<ChartObject> {
        self.inner.list_charts()
    }

    /// See [`UpdateGroup::dependency_indexing_status_requirement`]
    pub fn dependency_indexing_status_requirement(
        &self,
        enabled_charts: &HashSet<ChartKey>,
    ) -> IndexingStatus {
        self.inner
            .dependency_indexing_status_requirement(enabled_charts)
    }

    /// See [`UpdateGroup::list_dependency_mutex_ids`]
    pub fn list_dependency_mutex_ids(&self) -> HashSet<String> {
        self.inner.list_dependency_mutex_ids()
    }

    /// See [`UpdateGroup::dependency_mutex_ids_of`]
    pub fn dependency_mutex_ids_of(&self, chart_id: &crate::ChartKey) -> Option<HashSet<String>> {
        self.inner.dependency_mutex_ids_of(chart_id)
    }
}

impl SyncUpdateGroup {
    /// Ignores missing elements
    fn joint_dependencies_mutex_ids_of(&self, charts: &HashSet<ChartKey>) -> HashSet<String> {
        let mut result = HashSet::new();
        for id in charts {
            let Some(dependencies_ids) = self.inner.dependency_mutex_ids_of(id) else {
                tracing::warn!(
                    update_group = self.name(),
                    "`dependency_mutex_ids_of` of member chart '{id}' returned `None`. Expected `Some(..)`"
                );
                continue;
            };
            result.extend(dependencies_ids.into_iter().map(|s| s.to_owned()))
        }
        result
    }

    async fn lock_in_order(&self, mut to_lock: HashSet<String>) -> Vec<MutexGuard<()>> {
        let mut guards = vec![];
        // .iter() is ordered by key, so order is followed
        for (name, mutex) in self.dependencies_mutexes.iter() {
            if to_lock.remove(name) {
                tracing::debug!(update_group = self.name(), mutex_id = name, "locking mutex");
                let guard = match mutex.try_lock() {
                    Ok(v) => v,
                    Err(_) => {
                        tracing::info!(
                            update_group = self.name(),
                            mutex_id = name,
                            "found locked update mutex, waiting for unlock"
                        );
                        mutex.lock().await
                    }
                };
                guards.push(guard);
            }
        }
        if !to_lock.is_empty() {
            tracing::warn!(
                update_group = self.name(),
                "did not lock mutexes for all dependencies, this might lead \
                to inaccuracies in calculations. missing mutexes for: {:?}",
                to_lock
            )
        }
        guards
    }

    // (enabled_members, enabled_members_with_deps)
    fn get_enabled_members_with_deps(
        &self,
        enabled_charts: &HashSet<ChartKey>,
    ) -> (HashSet<ChartKey>, HashSet<String>) {
        let members: HashSet<ChartKey> = self
            .list_charts()
            .into_iter()
            .map(|c| c.properties.key)
            .collect();
        // in-place intersection
        let enabled_members: HashSet<ChartKey> = members
            .into_iter()
            .filter(|m| enabled_charts.contains(m))
            .collect();
        let enabled_members_with_deps_mutex_ids =
            self.joint_dependencies_mutex_ids_of(&enabled_members);
        (enabled_members, enabled_members_with_deps_mutex_ids)
    }

    /// Lock only enabled charts (that are also group members) and their dependencies
    ///
    /// Returns joint mutex guard and enabled group members list
    async fn lock_enabled_and_dependencies(
        &self,
        enabled_charts: &HashSet<ChartKey>,
    ) -> ChartsMutexGuards {
        let (enabled_members, enabled_members_with_deps) =
            self.get_enabled_members_with_deps(enabled_charts);
        // order is very important to prevent deadlocks
        let joint_guard = self.lock_in_order(enabled_members_with_deps).await;
        ChartsMutexGuards {
            _mutexes_for_charts_with_dependenices: joint_guard,
            charts: enabled_members,
        }
    }

    /// Ignores unknown names
    ///
    /// Performs [`UpdateGroup::create_charts`] synchronizing w/ other actions
    /// using mutexes
    pub async fn create_charts_sync(
        &self,
        db: &DatabaseConnection,
        creation_time_override: Option<chrono::DateTime<Utc>>,
        enabled_charts: &HashSet<ChartKey>,
    ) -> Result<(), ChartError> {
        let charts_locked_mutexes = self.lock_enabled_and_dependencies(enabled_charts).await;
        self.inner
            .create_charts(db, creation_time_override, charts_locked_mutexes.charts())
            .await
            .map_err(ChartError::StatsDB)
    }

    /// Ignores unknown names
    ///
    /// Performs [`UpdateGroup::update_charts`] synchronizing w/ other actions
    /// using mutexes
    pub async fn update_charts_sync(
        &self,
        params: UpdateParameters<'_>,
        enabled_charts: &HashSet<ChartKey>,
    ) -> Result<(), ChartError> {
        let locked_enabled_members = self.lock_enabled_and_dependencies(enabled_charts).await;
        tracing::info!(
            update_group = self.name(),
            "updating group with enabled members {:?}",
            locked_enabled_members.charts()
        );
        self.inner
            .update_charts(params, locked_enabled_members.charts())
            .await
    }

    /// Ignores unknown names
    ///
    /// Performs [`UpdateGroup::set_next_update_from`] synchronizing w/ other actions
    /// using mutexes
    pub async fn set_next_update_from_sync(
        &self,
        db: &DatabaseConnection,
        from: NaiveDate,
        enabled_charts: &HashSet<ChartKey>,
    ) -> Result<(), ChartError> {
        let locked_enabled_members = self.lock_enabled_and_dependencies(enabled_charts).await;
        tracing::info!(
            update_group = self.name(),
            "setting next update from {} for members {:?}",
            from,
            locked_enabled_members.charts()
        );
        self.inner
            .set_next_update_from(db, from, locked_enabled_members.charts())
            .await
    }

    /// Ignores non-member charts
    pub fn enabled_members_with_deps(
        &self,
        enabled_charts: &HashSet<ChartKey>,
    ) -> HashSet<ChartKey> {
        let mut result = HashSet::new();
        for id in enabled_charts {
            let Some(dependencies_ids) = self.inner.dependency_keys_of(id) else {
                tracing::warn!(
                    update_group = self.name(),
                    "`dependency_mutex_ids_of` of member chart '{id}' returned `None`. Expected `Some(..)`"
                );
                continue;
            };
            result.extend(dependencies_ids.into_iter().map(|s| s.to_owned()))
        }
        result
    }
}

/// Mutexes for the listed charts as well as their
/// dependencies
pub struct ChartsMutexGuards<'a> {
    _mutexes_for_charts_with_dependenices: Vec<MutexGuard<'a, ()>>,
    charts: HashSet<ChartKey>,
}

impl ChartsMutexGuards<'_> {
    /// Charts for which the mutexes were locked in the first place
    pub fn charts(&self) -> &HashSet<ChartKey> {
        &self.charts
    }

    /// Extends content of current guard with another
    pub fn extend(&mut self, other: Self) {
        self._mutexes_for_charts_with_dependenices
            .extend(other._mutexes_for_charts_with_dependenices);
        self.charts.extend(other.charts);
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};
    use tokio::sync::Mutex;

    use crate::{
        data_source::DataSource,
        lines::{ContractsGrowth, NewContracts},
        update_group::InitializationError,
    };

    use super::SyncUpdateGroup;

    construct_update_group!(GroupWithoutDependencies {
        charts: [ContractsGrowth],
    });

    #[test]
    fn new_checks_mutexes() {
        let mutexes: BTreeMap<String, Arc<Mutex<()>>> = [(
            ContractsGrowth::mutex_id().unwrap(),
            Arc::new(Mutex::new(())),
        )]
        .into();

        // need for consistent comparison of errors
        fn sorted_init_error(e: InitializationError) -> InitializationError {
            let InitializationError {
                mut missing_mutexes,
            } = e;
            missing_mutexes.sort();
            InitializationError { missing_mutexes }
        }

        assert_eq!(
            SyncUpdateGroup::new(&mutexes, Arc::new(GroupWithoutDependencies))
                .map_err(sorted_init_error)
                .unwrap_err(),
            sorted_init_error(InitializationError {
                missing_mutexes: vec![NewContracts::mutex_id().unwrap(),]
            })
        );
    }
}
