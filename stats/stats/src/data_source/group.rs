use std::collections::HashSet;

use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr};

use crate::UpdateError;

// todo: reconsider name of module (also should help reading??)

// todo: move comments somewhere (to module likely)
/// Directed Acyclic Connected Graph
pub trait UpdateGroup<P> {
    async fn create_charts(
        db: &DatabaseConnection,
        enabled_names: &HashSet<String>,
        current_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr>;
    async fn update_charts(params: P, enabled_names: &HashSet<String>) -> Result<(), UpdateError>;
}

/// Since group member types are different (and trait impls have different associated types),
/// we can't use homogeneous collections like `Vec`.
///
/// Therefore, the macro helps to avoid boilerplate when defining the update groups.
#[macro_export]
macro_rules! construct_update_group {
    ($group_name:ident = [
        $($member:path),*
        $(,)?
    ]) => {
        pub struct $group_name;

        impl<'a>
            $crate::data_source::group::UpdateGroup<
                $crate::data_source::types::UpdateParameters<'a>,
            > for $group_name
        {
            async fn create_charts(
                #[allow(unused)]
                db: &sea_orm::DatabaseConnection,
                #[allow(unused)]
                enabled_names: &std::collections::HashSet<String>,
                #[allow(unused)]
                current_time: &chrono::DateTime<chrono::Utc>,
            ) -> Result<(), sea_orm::DbErr> {
                $(
                    if enabled_names.contains(<$member>::name()) {
                        <$member>::init_all_locally(db, current_time).await?;
                    }
                )*
                Ok(())
            }

            async fn update_charts(
                params: $crate::data_source::types::UpdateParameters<'a>,
                #[allow(unused)]
                enabled_names: &std::collections::HashSet<String>,
            ) -> Result<(), $crate::UpdateError> {
                #[allow(unused)]
                let cx = $crate::data_source::types::UpdateContext::<$crate::data_source::types::UpdateParameters<'a>>::from_inner(
                    params.into()
                );
                $(
                    if enabled_names.contains(<$member>::name()) {
                        <$member>::update_from_remote(&cx).await?;
                    }
                )*
                Ok(())
            }
        }
    };
}
