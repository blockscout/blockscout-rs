//! Common utilities used across statistics

use std::ops::RangeInclusive;

use chrono::{NaiveDate, NaiveTime};
use sea_orm::{prelude::DateTimeUtc, Value};

pub fn day_start(date: NaiveDate) -> DateTimeUtc {
    date.and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("correct time"))
        .and_utc()
}

/// `blocks` table should be aliased as `b`.
///
/// No other values are expected to be in the query.
pub fn block_timestamp_sql_filter(
    range: Option<RangeInclusive<DateTimeUtc>>,
) -> (String, Vec<Value>) {
    if let Some(range) = range {
        (
            r#"AND
                b.timestamp < $2 AND
                b.timestamp >= $1"#
                .to_owned(),
            vec![range.start().into(), range.end().into()],
        )
    } else {
        ("".to_owned(), vec![])
    }
}
