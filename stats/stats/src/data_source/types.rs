use chrono::Utc;
use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct UpdateParameters<'a> {
    pub db: &'a DatabaseConnection,
    pub blockscout: &'a DatabaseConnection,
    /// If `None`, it will be measured at the start of update
    /// (i.e. after taking mutexes)
    pub update_time_override: Option<chrono::DateTime<Utc>>,
    /// Force full re-update
    pub force_full: bool,
}

#[derive(Clone)]
pub struct UpdateContext<'a> {
    pub db: &'a DatabaseConnection,
    pub blockscout: &'a DatabaseConnection,
    /// Update time
    pub time: chrono::DateTime<Utc>,
    pub force_full: bool,
}

impl<'a> UpdateContext<'a> {
    pub fn from_params_now_or_override(value: UpdateParameters<'a>) -> Self {
        Self {
            db: value.db,
            blockscout: value.blockscout,
            time: value.update_time_override.unwrap_or_else(Utc::now),
            force_full: value.force_full,
        }
    }
}

// todo: assoc type
pub trait Get {
    type Value;
    fn get() -> Self::Value;
}

/// Usage:
/// ```
/// # use stats::gettable_const;
/// # use crate::stats::data_source::types::Get;
/// gettable_const!(ConstName: u64 = 123);
///
/// fn get_value_example() -> u64 {
///     ConstName::get()
/// }
/// ```
#[macro_export]
macro_rules! gettable_const {
    ($name:ident: $type:ty = $value:expr) => {
        pub struct $name;
        impl $crate::data_source::types::Get for $name {
            type Value = $type;
            fn get() -> $type {
                $value
            }
        }
    };
}
