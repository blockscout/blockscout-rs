use std::{
    collections::{BTreeMap, HashSet},
    marker::{Send, Sync},
    ops::Deref,
    sync::Arc,
    vec::Vec,
};

use async_trait::async_trait;
use chrono::Utc;
use itertools::Itertools;
use sea_orm::{DatabaseConnection, DbErr};
use thiserror::Error;
use tokio::sync::{Mutex, MutexGuard};
use tracing::warn;

use crate::{ChartDynamic, UpdateError};

use super::types::UpdateParameters;

// todo: reconsider name of module (also should help reading??)

#[derive(Error, Debug)]
#[error("Could not initialize update group: mutexes for {missing_mutexes:?} were not provided")]
pub struct InitializationError {
    pub missing_mutexes: Vec<String>,
}

// todo: move comments somewhere (to module likely)
// todo: use `trait-variant` once updated, probably
// `async_trait` allows making trait objects
/// Directed Acyclic Connected Graph
#[async_trait]
pub trait UpdateGroup {
    // &self is only to make fns dispatchable (and trait to be object-safe)
    /// Group name
    fn name(&self) -> String;
    /// List names of charts - members of the group.
    fn list_charts(&self) -> Vec<ChartDynamic>;
    /// List mutex ids of group members + their dependencies.
    /// Dependencies participate in updates, thus access to them needs to be
    /// synchronized.
    fn list_dependency_mutex_ids(&self) -> HashSet<&'static str>;
    /// List mutex ids of particular group member dependencies (including the member itself).
    ///
    /// `None` if `chart_name` is not a member.
    fn dependency_mutex_ids_of(&self, chart_name: &str) -> Option<HashSet<&'static str>>;
    // todo: comments
    async fn create_charts(
        &self,
        db: &DatabaseConnection,
        creation_time_override: Option<chrono::DateTime<Utc>>,
        enabled_names: &HashSet<String>,
    ) -> Result<(), DbErr>;
    // todo: comments
    async fn update_charts<'a>(
        &self,
        params: UpdateParameters<'a>,
        enabled_names: &HashSet<String>,
    ) -> Result<(), UpdateError>;
}

pub mod macro_reexport {
    pub use chrono;
    pub use paste;
}

// todo: move example to module?
// todo: add check for unique names
// steps:
// - make macro for making all groups altogether.
// - in the macro make smth like `validate()` that will check uniqueness of names with the help of
// `TypeId` (https://doc.rust-lang.org/beta/core/any/struct.TypeId.html#method.of)
// and (probably) phf https://docs.rs/phf/latest/phf/index.html
// - mark "TODO" when `const` version of `TypeId::of` gets stabilized (to check it in compile-time w/ const-assert)
// - do the same for update group names

/// Construct update group that implemants [`UpdateGroup`]. The main purpose of the
/// group is to update its members together.
///
/// All membere must implement [`crate::Chart`] and [`crate::data_source::source::DataSource`].
///
/// The behaviour is the following:
/// 1. when `create` or `update` is triggered, each member's correspinding method is triggered
/// 2. the method recursively updates/creates its dependencies
///
/// In addition, to update synchronization, the resulting group provides object-safe interface
/// for interacting with its members (charts). In particular, the charts themselves
/// intentionally do not have `self` parameter, which is present in the resulting group.
///
/// ## Usage
///
/// Since `update` and `create` are performed recursively it makes sense to include
/// all ancestors of members (i.e. dependencies, dependencies of them, etc.) into the group.
/// Otherwise the following may occur:
///
/// Let's say we have chart `A` that depends on `B`. We make an update group with only `A`.
/// See possible configurations for chart availability (e.g. for data requests):
/// - Both `A` and `B` are enabled:\
/// Update/create of `A` is called which triggers `B`. Everything is fine.
/// - `A` is on, `B` is off:\
/// Update/create of `A` is called which triggers `B`, as intended. Everything is fine.
/// - `A` is off, `B` is on:\
/// Group only contains `A`, which means nothing is triggered. Quite counter-intuitive.
/// - `A` is off, `B` is off:\
/// Nothing happens, as expected.
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
/// # use stats::{Chart, construct_update_group, DateValue, UpdateError};
/// # use stats::data_source::{
/// #     kinds::{
/// #         chart::{
/// #             BatchUpdateableChartWrapper, RemoteChart, RemoteChartWrapper,
/// #             UpdateableChartWrapper,
/// #         },
/// #         remote::RemoteSource
/// #     },
/// #     types::{UpdateContext, UpdateParameters},
/// # };
/// # use chrono::NaiveDate;
/// # use entity::sea_orm_active_enums::ChartType;
/// # use std::ops::RangeInclusive;
///
/// struct DummyRemoteSource;
///
/// impl RemoteSource for DummyRemoteSource {
///     fn get_query(from: NaiveDate, to: NaiveDate) -> sea_orm::Statement {
///         // not called
///         unimplemented!()
///     }
///     async fn query_data(
///         cx: &UpdateContext<'_>,
///         range: RangeInclusive<NaiveDate>,
///     ) -> Result<Vec<DateValue>, UpdateError> {
///         Ok(vec![])
///     }
/// }
///
/// struct DummyChart;
///
/// impl Chart for DummyChart {
///     const NAME: &'static str = "dummyChart";
///     fn chart_type() -> ChartType {
///         ChartType::Line
///     }
/// }
///
/// impl RemoteChart for DummyChart {
///     type Dependency = DummyRemoteSource;
/// }
///
/// // use wrappers to utilize existing implementation of `DataSource`
/// // `Chart` impl is delegated to `DummyChart`.
/// type DummyChartSource =
///     UpdateableChartWrapper<BatchUpdateableChartWrapper<RemoteChartWrapper<DummyChart>>>;
///
/// construct_update_group!(ExampleUpdateGroup {
///     name: "exampleGroup",
///     charts: [DummyChartSource],
/// });
/// ```
///
#[macro_export]
macro_rules! construct_update_group {
    ($group_name:ident {
        name: $name:literal,
        charts: [
            $($member:path),*
            $(,)?
        ] $(,)?
    }) => {
        pub struct $group_name;

        #[::async_trait::async_trait]
        impl $crate::data_source::group::UpdateGroup for $group_name
        {
            fn name(&self) -> ::std::string::String {
                $name.into()
            }

            fn list_charts(&self) -> ::std::vec::Vec<$crate::ChartDynamic> {
                std::vec![
                    $(
                        $crate::ChartDynamic::construct_from_chart::<$member>(),
                    )*
                ]
            }

            fn list_dependency_mutex_ids(&self) -> ::std::collections::HashSet<&'static str> {
                let mut ids = ::std::collections::HashSet::new();
                $(
                    ids.extend(<$member as $crate::data_source::source::DataSource>::all_dependencies_mutex_ids().into_iter());
                )*
                ids
            }

            fn dependency_mutex_ids_of(&self, chart_name: &str) -> Option<::std::collections::HashSet<&'static str>> {
                $(
                    if chart_name == <$member as $crate::Chart>::NAME {
                        return Some(<$member as $crate::data_source::source::DataSource>::all_dependencies_mutex_ids());
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
                    $crate::data_source::group::macro_reexport::chrono::DateTime<
                        $crate::data_source::group::macro_reexport::chrono::Utc
                    >
                >,
                #[allow(unused)]
                enabled_names: &::std::collections::HashSet<String>,
            ) -> Result<(), sea_orm::DbErr> {
                let current_time = creation_time_override.unwrap_or_else(|| $crate::data_source::group::macro_reexport::chrono::Utc::now());
                $(
                    if enabled_names.contains(<$member as $crate::Chart>::NAME) {
                        <$member as $crate::data_source::source::DataSource>::init_all_locally(db, &current_time).await?;
                    }
                )*
                Ok(())
            }

            async fn update_charts<'a>(
                &self,
                params: $crate::data_source::types::UpdateParameters<'a>,
                #[allow(unused)]
                enabled_names: &::std::collections::HashSet<String>,
            ) -> Result<(), $crate::UpdateError> {
                #[allow(unused)]
                let cx: $crate::data_source::types::UpdateContext = params.into();
                $(
                    if enabled_names.contains(<$member as $crate::Chart>::NAME) {
                        <$member as $crate::data_source::source::DataSource>::update_from_remote(&cx).await?;
                    }
                )*
                Ok(())
            }
        }

    };
}

pub type ArcUpdateGroup = Arc<dyn for<'a> UpdateGroup + Send + Sync + 'static>;

/// Synchronized update group.
///
/// ## Deadlock-free
/// Deadlock-free, as long as only these groups are used
/// and mapping name-mutex is consistent between the groups.
///
/// This is achieved using deadlock ordering.
/// For more info see [link, section "lock ordering"](https://www.cs.cornell.edu/courses/cs4410/2017su/lectures/lec09-deadlock.html)
///
/// The order is a lexicographical order of chart (data source) mutex IDs
#[derive(Clone)]
pub struct SyncUpdateGroup {
    /// Mutexes. Acquired in lexicographical order (=order within `BTreeMap`)
    dependencies_mutexes: BTreeMap<String, Arc<Mutex<()>>>,
    inner: ArcUpdateGroup,
}

impl SyncUpdateGroup {
    /// `chart_mutexes` must contain mutexes for all members of the group + their dependencies
    pub fn new(
        all_chart_mutexes: &BTreeMap<String, Arc<Mutex<()>>>,
        inner: ArcUpdateGroup,
    ) -> Result<Self, InitializationError>
    where
        Self: Sized,
    {
        let dependencies: HashSet<&str> = inner.list_dependency_mutex_ids();
        let received_charts: HashSet<&str> =
            all_chart_mutexes.keys().map(|n| (*n).deref()).collect();
        let missing_mutexes = dependencies
            .difference(&received_charts)
            .into_iter()
            .map(|s| (*s).to_owned())
            .collect_vec();

        let mut dependencies_mutexes = BTreeMap::new();

        for dependency_name in dependencies {
            let mutex = all_chart_mutexes
                .get(dependency_name)
                .ok_or(InitializationError {
                    missing_mutexes: missing_mutexes.clone(),
                })?;
            dependencies_mutexes.insert(dependency_name.to_owned(), mutex.clone());
        }
        return Ok(Self {
            dependencies_mutexes,
            inner: inner,
        });
    }

    /// See [`UpdateGroup::name`]
    pub fn name(&self) -> String {
        self.inner.name()
    }

    /// See [`UpdateGroup::list_charts``]
    pub fn list_charts(&self) -> Vec<ChartDynamic> {
        self.inner.list_charts()
    }

    /// See [`UpdateGroup::list_dependency_mutex_ids`]
    pub fn list_dependency_mutex_ids(&self) -> HashSet<&'static str> {
        self.inner.list_dependency_mutex_ids()
    }

    /// See [`UpdateGroup::dependency_mutex_ids_of`]
    pub fn dependency_mutex_ids_of(&self, chart_name: &str) -> Option<HashSet<&'static str>> {
        self.inner.dependency_mutex_ids_of(chart_name)
    }
}

impl SyncUpdateGroup {
    /// Ignores missing elements
    fn joint_dependencies_of(&self, chart_names: &HashSet<String>) -> HashSet<String> {
        let mut result = HashSet::new();
        for name in chart_names {
            let Some(dependencies_ids) = self.inner.dependency_mutex_ids_of(name) else {
                warn!(
                    update_group=self.name(),
                    "`dependency_mutex_ids_of` of member chart '{name}' returned `None`. Expected `Some(..)`"
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
                let guard = match mutex.try_lock() {
                    Ok(v) => v,
                    Err(_) => {
                        tracing::warn!(
                            update_group = self.name(),
                            chart_name = name,
                            "found locked update mutex, waiting for unlock"
                        );
                        mutex.lock().await
                    }
                };
                guards.push(guard);
            }
        }
        guards
    }

    /// Lock only enabled charts and their dependencies
    ///
    /// Returns (joint mutex guard) and (enabled group members list)
    async fn lock_enabled_dependencies(
        &self,
        enabled_names: &HashSet<String>,
    ) -> (Vec<MutexGuard<()>>, HashSet<String>) {
        let members: HashSet<String> = self.list_charts().into_iter().map(|c| c.name).collect();
        // in-place intersection
        let enabled_members: HashSet<String> = members
            .into_iter()
            .filter(|m| enabled_names.contains(m))
            .collect();
        let enabled_members_with_deps = self.joint_dependencies_of(&enabled_members);
        // order is very important to prevent deadlocks
        let joint_guard = self.lock_in_order(enabled_members_with_deps).await;
        (joint_guard, enabled_members)
    }

    /// Ignores unknown names
    pub async fn create_charts_with_mutexes<'a>(
        &self,
        db: &DatabaseConnection,
        creation_time_override: Option<chrono::DateTime<Utc>>,
        enabled_names: &HashSet<String>,
    ) -> Result<(), UpdateError> {
        let (_joint_guard, enabled_members) = self.lock_enabled_dependencies(enabled_names).await;
        self.inner
            .create_charts(db, creation_time_override, &enabled_members)
            .await
            .map_err(UpdateError::StatsDB)
    }

    /// Ignores unknown names
    pub async fn update_charts_with_mutexes<'a>(
        &self,
        params: UpdateParameters<'a>,
        enabled_names: &HashSet<String>,
    ) -> Result<(), UpdateError> {
        let (_joint_guard, enabled_members) = self.lock_enabled_dependencies(enabled_names).await;
        self.inner.update_charts(params, &enabled_members).await
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn new_checks_mutexes() {
        // todo: test
    }
}
