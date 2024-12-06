//! Common utilities used across statistics

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use sea_orm::Value;
use std::{ops::Range, sync::Arc};

// this const is not public in `chrono` for some reason
pub const NANOS_PER_SEC: i32 = 1_000_000_000;

pub fn day_start(date: &NaiveDate) -> DateTime<Utc> {
    date.and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("correct time"))
        .and_utc()
}

/// Database connection with a mark of what database it is.
/// Used to separate caching for different databases to
/// prevent data clashes when running unit tests concurrently.
#[derive(Debug, Clone)]
pub struct MarkedDbConnection {
    pub connection: Arc<sea_orm::DatabaseConnection>,
    pub db_name: String,
}

impl MarkedDbConnection {
    #[cfg(any(feature = "test-utils", test))]
    pub fn from_test_db(
        guard: &blockscout_service_launcher::test_database::TestDbGuard,
    ) -> Option<Self> {
        Some(Self {
            connection: guard.client(),
            db_name: guard.db_url().split("/").last()?.to_owned(),
        })
    }

    pub fn main_connection(inner: Arc<sea_orm::DatabaseConnection>) -> Self {
        Self {
            connection: inner,
            db_name: "main".to_owned(),
        }
    }

    pub fn in_memory(inner: Arc<sea_orm::DatabaseConnection>) -> Self {
        Self {
            connection: inner,
            db_name: "in_memory".to_owned(),
        }
    }
}

/// Used inside [`sql_with_range_filter_opt`]
///
/// `filter_arg_number_start = len(arg)+1 // (length of other args + 1)`
/// `filter_by` - column/property(?) name in SQL
///
/// ### Results
/// Vec should be appended to the args.
/// String should be inserted in places for filter.
pub(crate) fn produce_filter_and_values(
    range: Option<Range<DateTime<Utc>>>,
    filter_by: &str,
    filter_arg_number_start: usize,
) -> (String, Vec<Value>) {
    if let Some(range) = range {
        let arg_n_1 = filter_arg_number_start;
        let arg_n_2 = arg_n_1 + 1;
        (
            format!(
                " AND
                {filter_by} < ${arg_n_2} AND
                {filter_by} >= ${arg_n_1}"
            ),
            vec![range.start.into(), range.end.into()],
        )
    } else {
        ("".to_owned(), vec![])
    }
}

// had to make macro because otherwise can't use `statement_with_filter_placeholder`
// in `format!` :(
/// Add filter statement, if `range` provided.
///
/// `statement_with_filter_placeholder` must have `filter` named parameter
/// `filter_by` is a column/property(?) in SQL used to generate string for `filter`
macro_rules! sql_with_range_filter_opt {
    (
        $db_backend: expr,
        $statement_with_filter_placeholder: literal,
        [$($value: expr),* $(,)?],
        $filter_by:expr,
        $range:expr $(,)?
    ) => {
        {
            let mut values = ::std::vec![ $($value),* ];
            let filter_arg_number_start = values.len()+1;
            let (filter_str, filter_values) = $crate::utils::produce_filter_and_values(
                $range, $filter_by, filter_arg_number_start
            );
            values.extend(filter_values.into_iter());
            let sql = ::std::format!(
                $statement_with_filter_placeholder,
                filter=filter_str,
            );
            ::sea_orm::Statement::from_sql_and_values($db_backend, &sql, values)
        }
    };
}

pub(crate) use sql_with_range_filter_opt;

#[cfg(test)]
mod test {
    use itertools::Itertools;
    use pretty_assertions::assert_eq;
    use sea_orm::{DbBackend, Statement};

    use super::*;

    /// In order to ignore spaces during comparison
    fn compact_sql(s: Statement) -> String {
        s.to_string().split_whitespace().join(" ")
    }

    #[test]
    fn filter_and_values_works() {
        assert_eq!(
            produce_filter_and_values(None, "aboba", 123),
            ("".to_string(), vec![])
        );

        let time_1 = DateTime::<Utc>::from_timestamp(1234567, 0).unwrap();
        let time_2 = DateTime::<Utc>::from_timestamp(7654321, 0).unwrap();
        assert_eq!(
            produce_filter_and_values(Some(time_1..time_2), "aboba", 123),
            (
                " AND
                aboba < $124 AND
                aboba >= $123"
                    .to_string(),
                vec![time_1.into(), time_2.into()]
            )
        );
    }

    const ETH: i64 = 1_000_000_000_000_000_000;

    fn naive_sql_selector(range: Option<Range<DateTime<Utc>>>) -> Statement {
        match range {
            Some(range) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    DATE(blocks.timestamp) as date,
                    (AVG(block_rewards.reward) / $1)::FLOAT as value
                FROM block_rewards
                JOIN blocks ON block_rewards.block_hash = blocks.hash
                WHERE 
                    blocks.timestamp != to_timestamp(0) AND 
                    blocks.consensus = true AND
                    blocks.timestamp < $3 AND
                    blocks.timestamp >= $2
                GROUP BY date
                "#,
                vec![ETH.into(), range.start.into(), range.end.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    DATE(blocks.timestamp) as date,
                    (AVG(block_rewards.reward) / $1)::FLOAT as value
                FROM block_rewards
                JOIN blocks ON block_rewards.block_hash = blocks.hash
                WHERE 
                    blocks.timestamp != to_timestamp(0) AND 
                    blocks.consensus = true
                GROUP BY date
                "#,
                vec![ETH.into()],
            ),
        }
    }

    #[test]
    fn sql_with_range_filter_empty_works() {
        let range = None;
        assert_eq!(
            compact_sql(naive_sql_selector(range.clone())),
            compact_sql(sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        DATE(blocks.timestamp) as date,
                        (AVG(block_rewards.reward) / $1)::FLOAT as value
                    FROM block_rewards
                    JOIN blocks ON block_rewards.block_hash = blocks.hash
                    WHERE 
                        blocks.timestamp != to_timestamp(0) AND 
                        blocks.consensus = true {filter}
                    GROUP BY date
                "#,
                [ETH.into()],
                "blocks.timestamp",
                range,
            ))
        );
    }

    #[test]
    fn sql_with_range_filter_some_works() {
        let range = Some(
            DateTime::<Utc>::from_timestamp(1234567, 0).unwrap()
                ..DateTime::<Utc>::from_timestamp(7654321, 0).unwrap(),
        );
        assert_eq!(
            compact_sql(naive_sql_selector(range.clone())),
            compact_sql(sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        DATE(blocks.timestamp) as date,
                        (AVG(block_rewards.reward) / $1)::FLOAT as value
                    FROM block_rewards
                    JOIN blocks ON block_rewards.block_hash = blocks.hash
                    WHERE 
                        blocks.timestamp != to_timestamp(0) AND 
                        blocks.consensus = true {filter}
                    GROUP BY date
                "#,
                [ETH.into()],
                "blocks.timestamp",
                range,
            ))
        );
    }
}
