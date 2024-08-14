pub mod db;
mod duration;
mod extended;
pub mod timespans;
mod traits;

use entity::chart_data;
use sea_orm::Set;

pub use duration::TimespanDuration;
pub use extended::ExtendedTimespanValue;
pub use traits::{ConsistsOf, Timespan, TimespanValueTrait, ZeroTimespanValue};

/// Some value for some time interval
#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimespanValue<T, V> {
    pub timespan: T,
    pub value: V,
}

impl<T> TimespanValue<T, String> {
    pub fn with_zero_value(timespan: T) -> Self {
        Self {
            timespan,
            value: "0".to_string(),
        }
    }
}

impl<T: Ord> TimespanValue<T, String> {
    pub fn relevant_or_zero(self, current_timespan: T) -> Self {
        if self.timespan < current_timespan {
            Self::with_zero_value(current_timespan)
        } else {
            self
        }
    }
}

impl<T: Timespan + Clone> TimespanValue<T, String> {
    pub fn active_model(
        &self,
        chart_id: i32,
        min_blockscout_block: Option<i64>,
    ) -> chart_data::ActiveModel {
        chart_data::ActiveModel {
            id: Default::default(),
            chart_id: Set(chart_id),
            date: Set(self.timespan.clone().into_date()),
            value: Set(self.value.clone()),
            created_at: Default::default(),
            min_blockscout_block: Set(min_blockscout_block),
        }
    }
}

#[macro_export]
macro_rules! impl_into_string_timespan_value {
    ($timespan:ty, $value:ty) => {
        impl From<$crate::charts::types::TimespanValue<$timespan, $value>>
            for $crate::charts::types::TimespanValue<$timespan, ::std::string::String>
        {
            fn from(value: $crate::charts::types::TimespanValue<$timespan, $value>) -> Self {
                Self {
                    timespan: value.timespan,
                    value: value.value.to_string(),
                }
            }
        }
    };
}
