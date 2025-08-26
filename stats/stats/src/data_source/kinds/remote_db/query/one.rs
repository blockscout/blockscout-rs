use std::marker::{PhantomData, Send};

use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::{FromQueryResult, Statement, TryGetable};

use crate::{
    ChartError,
    charts::db_interaction::read::{cached::find_one_value_cached, find_one_value},
    data_source::{
        kinds::remote_db::{RemoteQueryBehaviour, db_choice::DatabaseChoice},
        types::{Cacheable, IndexerMigrations, UpdateContext, WrappedValue},
    },
    range::{UniversalRange, inclusive_range_to_exclusive},
    types::{Timespan, TimespanValue},
    utils::interval_24h,
};

use super::StatementFromRange;

pub trait StatementForOne: DatabaseChoice {
    fn get_statement(completed_migrations: &IndexerMigrations) -> Statement;
}

/// Get a single record from remote (blockscout) DB using statement
/// `S`.
///
/// `P` - Type of point to retrieve within query.
/// `DateValue<String>` can be used to avoid parsing the values,
/// but `DateValue<Decimal>` or other types can be useful sometimes.
pub struct PullOne<S, Value>(PhantomData<(S, Value)>)
where
    S: StatementForOne,
    Value: FromQueryResult + Send;

impl<S, Value> RemoteQueryBehaviour for PullOne<S, Value>
where
    S: StatementForOne,
    Value: FromQueryResult + Send,
{
    type Output = Value;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Value, ChartError> {
        let statement = S::get_statement(&cx.indexer_applied_migrations);
        let data = find_one_value::<_, Value>(S::get_db(cx)?, statement)
            .await?
            .ok_or_else(|| ChartError::Internal("query returned nothing".into()))?;
        Ok(data)
    }
}

pub trait StatementFromUpdateTime: DatabaseChoice {
    fn get_statement(
        update_time: DateTime<Utc>,
        completed_migrations: &IndexerMigrations,
    ) -> Statement;
}

/// Just like `PullOne` but timespan is taken from update time
pub struct PullOneNowValue<S, Resolution, Value>(PhantomData<(S, Resolution, Value)>)
where
    S: StatementFromUpdateTime,
    Resolution: Timespan + Ord + Send,
    Value: Send + TryGetable;

impl<S, Resolution, Value> RemoteQueryBehaviour for PullOneNowValue<S, Resolution, Value>
where
    S: StatementFromUpdateTime,
    Resolution: Timespan + Ord + Send,
    Value: Send + TryGetable,
{
    type Output = TimespanValue<Resolution, Value>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<TimespanValue<Resolution, Value>, ChartError> {
        let statement = S::get_statement(cx.time, &cx.indexer_applied_migrations);
        let timespan = Resolution::from_date(cx.time.date_naive());
        let value = find_one_value::<_, WrappedValue<Value>>(S::get_db(cx)?, statement)
            .await?
            .ok_or_else(|| ChartError::Internal("query returned nothing".into()))?
            .value;
        Ok(TimespanValue { timespan, value })
    }
}

/// Will reuse result for the same produced query **within one update**
/// (based on update context)
pub struct PullOne24hCached<S, Value>(PhantomData<(S, Value)>)
where
    S: StatementFromRange,
    Value: FromQueryResult + Cacheable + Clone + Send;

impl<S, Value> RemoteQueryBehaviour for PullOne24hCached<S, Value>
where
    S: StatementFromRange,
    Value: FromQueryResult + Cacheable + Clone + Send,
{
    type Output = TimespanValue<NaiveDate, Value>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<TimespanValue<NaiveDate, Value>, ChartError> {
        let update_time = cx.time;
        let range_24h = interval_24h(update_time);
        let query = S::get_statement(
            Some(inclusive_range_to_exclusive(range_24h)),
            &cx.indexer_applied_migrations,
            &cx.enabled_update_charts_recursive,
        );

        let value = find_one_value_cached(S::get_db(cx)?, &cx.cache, query)
            .await?
            .ok_or_else(|| ChartError::Internal("query returned nothing".into()))?;

        Ok(TimespanValue {
            timespan: update_time.date_naive(),
            value,
        })
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::{BTreeMap, HashSet},
        ops::Range,
    };

    use chrono::{DateTime, Utc};
    use pretty_assertions::assert_eq;
    use sea_orm::{DatabaseBackend, DbBackend, MockDatabase, Statement};

    use crate::{
        ChartKey,
        data_source::{
            UpdateContext, UpdateParameters,
            kinds::remote_db::{
                RemoteQueryBehaviour, StatementFromRange,
                db_choice::{DatabaseChoice, UsePrimaryDB},
            },
            types::{IndexerMigrations, WrappedValue},
        },
        range::UniversalRange,
        tests::point_construction::dt,
        types::TimespanValue,
    };

    use super::PullOne24hCached;

    struct TestStatement;
    impl DatabaseChoice for TestStatement {
        type DB = UsePrimaryDB;
    }
    impl StatementFromRange for TestStatement {
        fn get_statement(
            _range: Option<Range<DateTime<Utc>>>,
            _completed_migrations: &IndexerMigrations,
            _enabled_update_charts_recursive: &HashSet<ChartKey>,
        ) -> Statement {
            Statement::from_string(DbBackend::Postgres, "SELECT id as value FROM t;")
        }
    }

    type TestedPullCached = PullOne24hCached<TestStatement, WrappedValue<String>>;

    #[tokio::test]
    async fn pull_caching_works() {
        let expected = WrappedValue {
            value: "value1".to_string(),
        };
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([
                // First query result
                vec![BTreeMap::from([(
                    "value",
                    sea_orm::Value::from(expected.value.clone()),
                )])],
                // Second query result
                vec![BTreeMap::from([("value", sea_orm::Value::from("value2"))])],
            ])
            .into_connection();
        let time = dt("2023-01-01T00:00:00").and_utc();
        let cx = UpdateContext::from_params_now_or_override(
            UpdateParameters::default_test_query_parameters(&db, &db, Some(time)),
        );
        assert_eq!(
            TimespanValue {
                timespan: time.date_naive(),
                value: expected.clone()
            },
            TestedPullCached::query_data(&cx, UniversalRange::full())
                .await
                .unwrap()
        );
        // the result is cached
        assert_eq!(
            TimespanValue {
                timespan: time.date_naive(),
                value: expected.clone()
            },
            TestedPullCached::query_data(&cx, UniversalRange::full())
                .await
                .unwrap()
        );
    }
}
