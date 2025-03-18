use std::marker::{PhantomData, Send};

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use sea_orm::{FromQueryResult, Statement, TryGetable};

use crate::{
    data_source::{
        kinds::remote_db::RemoteQueryBehaviour,
        types::{BlockscoutMigrations, Cacheable, UpdateContext, WrappedValue},
    },
    range::{inclusive_range_to_exclusive, UniversalRange},
    types::{Timespan, TimespanValue},
    ChartError,
};

use super::StatementFromRange;

pub trait StatementForOne {
    fn get_statement(completed_migrations: &BlockscoutMigrations) -> Statement;
}

/// Get a single record from remote (blockscout) DB using statement
/// `S`.
///
/// `P` - Type of point to retrieve within query.
/// `DateValue<String>` can be used to avoid parsing the values,
/// but `DateValue<Decimal>` or other types can be useful sometimes.
pub struct PullOne<S, Resolution, Value>(PhantomData<(S, Resolution, Value)>)
where
    S: StatementForOne,
    Resolution: Ord + Send,
    Value: Send,
    TimespanValue<Resolution, Value>: FromQueryResult;

impl<S, Resolution, Value> RemoteQueryBehaviour for PullOne<S, Resolution, Value>
where
    S: StatementForOne,
    Resolution: Ord + Send,
    Value: Send,
    TimespanValue<Resolution, Value>: FromQueryResult,
{
    type Output = TimespanValue<Resolution, Value>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<TimespanValue<Resolution, Value>, ChartError> {
        let query = S::get_statement(&cx.blockscout_applied_migrations);
        let data = TimespanValue::<Resolution, Value>::find_by_statement(query)
            .one(cx.blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?
            .ok_or_else(|| ChartError::Internal("query returned nothing".into()))?;
        Ok(data)
    }
}

pub trait StatementFromUpdateTime {
    fn get_statement(
        update_time: DateTime<Utc>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement;
}

/// Just like `PullOne` but timespan is taken from update time
pub struct PullOneValue<S, Resolution, Value>(PhantomData<(S, Resolution, Value)>)
where
    S: StatementFromUpdateTime,
    Resolution: Timespan + Ord + Send,
    Value: Send + TryGetable;

impl<S, Resolution, Value> RemoteQueryBehaviour for PullOneValue<S, Resolution, Value>
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
        let query = S::get_statement(cx.time, &cx.blockscout_applied_migrations);
        let timespan = Resolution::from_date(cx.time.date_naive());
        let value = WrappedValue::<Value>::find_by_statement(query)
            .one(cx.blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?
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
        let range_24h = update_time
            .checked_sub_signed(TimeDelta::hours(24))
            .unwrap_or(DateTime::<Utc>::MIN_UTC)..=update_time;
        let query = S::get_statement(
            Some(inclusive_range_to_exclusive(range_24h)),
            &cx.blockscout_applied_migrations,
        );

        let value = if let Some(cached) = cx.cache.get::<Value>(&query).await {
            cached
        } else {
            let find_by_statement = Value::find_by_statement(query.clone());
            let value = find_by_statement
                .one(cx.blockscout)
                .await
                .map_err(ChartError::BlockscoutDB)?
                .ok_or_else(|| ChartError::Internal("query returned nothing".into()))?;
            cx.cache.insert(&query, value.clone()).await;
            value
        };

        Ok(TimespanValue {
            timespan: update_time.date_naive(),
            value,
        })
    }
}

#[cfg(test)]
mod test {
    use std::{collections::BTreeMap, ops::Range};

    use chrono::{DateTime, Utc};
    use pretty_assertions::assert_eq;
    use sea_orm::{DatabaseBackend, DbBackend, MockDatabase, Statement};

    use crate::{
        data_source::{
            kinds::remote_db::{RemoteQueryBehaviour, StatementFromRange},
            types::{BlockscoutMigrations, WrappedValue},
            UpdateContext, UpdateParameters,
        },
        range::UniversalRange,
        tests::point_construction::dt,
        types::TimespanValue,
    };

    use super::PullOne24hCached;

    struct TestStatement;
    impl StatementFromRange for TestStatement {
        fn get_statement(
            _range: Option<Range<DateTime<Utc>>>,
            _completed_migrations: &BlockscoutMigrations,
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
        let cx = UpdateContext::from_params_now_or_override(UpdateParameters {
            db: &db,
            blockscout: &db,
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            update_time_override: Some(time),
            force_full: false,
        });
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
