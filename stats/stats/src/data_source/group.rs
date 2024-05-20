use std::collections::HashSet;

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr};

use crate::{ChartDynamic, UpdateError};

use super::types::UpdateParameters;

// todo: reconsider name of module (also should help reading??)

// todo: move comments somewhere (to module likely)
// todo: use `trait-variant` once updated, probably
// `async_trait` allows making trait objects
/// Directed Acyclic Connected Graph
#[async_trait]
pub trait UpdateGroup {
    // &self is only to make fns dispatchable (and trait to be object-safe)
    fn name(&self) -> String;
    fn list_charts(&self) -> std::vec::Vec<ChartDynamic>;
    async fn create_charts(
        &self,
        db: &DatabaseConnection,
        enabled_names: &HashSet<String>,
        current_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr>;
    async fn update_charts<'a>(
        &self,
        params: UpdateParameters<'a>,
        enabled_names: &HashSet<String>,
    ) -> Result<(), UpdateError>;
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
///     async fn query_data(
///         cx: &UpdateContext<UpdateParameters<'_>>,
///         range: RangeInclusive<NaiveDate>,
///     ) -> Result<Vec<DateValue>, UpdateError> {
///         Ok(vec![])
///     }
/// }
///
/// struct DummyChart;
///
/// impl Chart for DummyChart {
///     fn name() -> &'static str {
///         "dummyChart"
///     }
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
/// construct_update_group!(
///     ExampleUpdateGroup = [
///         DummyChartSource
///     ]
/// );
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
            async fn create_charts(
                &self,
                #[allow(unused)]
                db: &sea_orm::DatabaseConnection,
                #[allow(unused)]
                enabled_names: &std::collections::HashSet<String>,
                #[allow(unused)]
                current_time: &chrono::DateTime<chrono::Utc>,
            ) -> Result<(), sea_orm::DbErr> {
                $(
                    if enabled_names.contains(<$member as $crate::Chart>::NAME) {
                        <$member as $crate::data_source::source::DataSource>::init_all_locally(db, current_time).await?;
                    }
                )*
                Ok(())
            }

            async fn update_charts<'a>(
                &self,
                params: $crate::data_source::types::UpdateParameters<'a>,
                #[allow(unused)]
                enabled_names: &std::collections::HashSet<String>,
            ) -> Result<(), $crate::UpdateError> {
                #[allow(unused)]
                let cx = $crate::data_source::types::UpdateContext::<$crate::data_source::types::UpdateParameters<'a>>::from_inner(
                    params.into()
                );
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
