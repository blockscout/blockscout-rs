use std::collections::HashSet;

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr};

use crate::UpdateError;

use super::types::UpdateParameters;

// todo: reconsider name of module (also should help reading??)

// todo: move comments somewhere (to module likely)
// todo: use `trait-variant` once updated, probably
// `async_trait` allows making trait objects
/// Directed Acyclic Connected Graph
#[async_trait]
pub trait UpdateGroup {
    // &self is only to make fns dispatchable (and trait to be object-safe)
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
/// Construct update group that implemants [`UpdateGroup`]. All members of the group are updated
/// simultaneously, meaning that each group usually consisits of interconnected members and
/// all interconnected members are included in group. However this is not enforced and can
/// be intentionally violated.
///
/// All membere must implement [`crate::Chart`] and [`crate::data_source::source::DataSource`].
///
/// The behaviour is the following:
/// 1. when `create` or `update` is triggered, each member's correspinding method is triggered
/// 2. the method recursively updates/creates its dependencies
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
    ($group_name:ident = [
        $($member:path),*
        $(,)?
    ]) => {
        pub struct $group_name;

        #[::async_trait::async_trait]
        impl $crate::data_source::group::UpdateGroup for $group_name
        {
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
                    if enabled_names.contains(<$member as $crate::Chart>::name()) {
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
                    if enabled_names.contains(<$member as $crate::Chart>::name()) {
                        <$member as $crate::data_source::source::DataSource>::update_from_remote(&cx).await?;
                    }
                )*
                Ok(())
            }
        }
    };
}
