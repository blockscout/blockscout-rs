use std::marker::{PhantomData, Send};

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use sea_orm::{FromQueryResult, Statement, TryGetable};

use crate::{
    data_source::{
        kinds::remote_db::RemoteQueryBehaviour,
        types::{BlockscoutMigrations, UpdateContext},
    },
    range::{inclusive_range_to_exclusive, UniversalRange},
    types::TimespanValue,
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
            .one(cx.blockscout.connection.as_ref())
            .await
            .map_err(ChartError::BlockscoutDB)?
            .ok_or_else(|| ChartError::Internal("query returned nothing".into()))?;
        Ok(data)
    }
}

#[derive(FromQueryResult)]
struct FromQueryValue<V: TryGetable> {
    value: V,
}

/// Retrieve a single value `Value` for the period of last 24h
/// according to provided statemnt `S`.
///
/// The point will be for the time of update
pub struct PullOne24h<S, Value>(PhantomData<(S, Value)>)
where
    S: StatementFromRange,
    Value: Send + TryGetable;

impl<S, Value> RemoteQueryBehaviour for PullOne24h<S, Value>
where
    S: StatementFromRange,
    Value: Send + TryGetable,
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

        let value = FromQueryValue::<Value>::find_by_statement(query)
            .one(cx.blockscout.connection.as_ref())
            .await
            .map_err(ChartError::BlockscoutDB)?
            .ok_or_else(|| ChartError::Internal("query returned nothing".into()))?;

        Ok(TimespanValue {
            timespan: update_time.date_naive(),
            value: value.value,
        })
    }
}
