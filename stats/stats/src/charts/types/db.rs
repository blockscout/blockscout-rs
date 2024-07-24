use chrono::NaiveDate;
use entity::chart_data;

use sea_orm::{FromQueryResult, Set, TryGetable};

use super::DateValue;

// Separate type instead of `TimespanValue` just to derive `FromQueryResult`
/// Internal (database) representation of data points.
///
/// Intended only for reusing the implementation of `FromQueryResult` for
/// particular cases of `TimespanValue`
#[derive(FromQueryResult, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DbDateValue<V: TryGetable> {
    pub date: NaiveDate,
    pub value: V,
}

impl<V: TryGetable> From<DbDateValue<V>> for DateValue<V> {
    fn from(value: DbDateValue<V>) -> Self {
        Self {
            timespan: value.date,
            value: value.value,
        }
    }
}

impl DbDateValue<String> {
    pub fn active_model(
        &self,
        chart_id: i32,
        min_blockscout_block: Option<i64>,
    ) -> chart_data::ActiveModel {
        chart_data::ActiveModel {
            id: Default::default(),
            chart_id: Set(chart_id),
            date: Set(self.date),
            value: Set(self.value.clone()),
            created_at: Default::default(),
            min_blockscout_block: Set(min_blockscout_block),
        }
    }
}
