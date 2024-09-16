use std::{
    marker::{PhantomData, Send},
    ops::Range,
};

use sea_orm::{prelude::DateTimeUtc, FromQueryResult, Statement};

use crate::{
    data_source::types::{BlockscoutMigrations, UpdateContext},
    types::TimespanValue,
    UpdateError,
};

use super::RemoteQueryBehaviour;

pub trait StatementFromRange {
    /// `completed_migrations` can be used for selecting more optimal query
    fn get_statement(
        range: Option<Range<DateTimeUtc>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement;
}

/// Pull data from remote (blockscout) db according to statement
/// `S` and sort it by date.
///
/// `P` - Type of point to retrieve within query.
/// `DateValue<String>` can be used to avoid parsing the values,
/// but `DateValue<Decimal>` or other types can be useful sometimes.
pub struct PullAllWithAndSort<S, Resolution, Value>(PhantomData<(S, Resolution, Value)>)
where
    S: StatementFromRange,
    Resolution: Ord + Send,
    Value: Send,
    TimespanValue<Resolution, Value>: FromQueryResult;

impl<S, Resolution, Value> RemoteQueryBehaviour for PullAllWithAndSort<S, Resolution, Value>
where
    S: StatementFromRange,
    Resolution: Ord + Send,
    Value: Send,
    TimespanValue<Resolution, Value>: FromQueryResult,
{
    type Output = Vec<TimespanValue<Resolution, Value>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
    ) -> Result<Vec<TimespanValue<Resolution, Value>>, UpdateError> {
        let query = S::get_statement(range, &cx.blockscout_applied_migrations);
        let mut data = TimespanValue::<Resolution, Value>::find_by_statement(query)
            .all(cx.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        // linear time for sorted sequences
        data.sort_unstable_by(|a, b| a.timespan.cmp(&b.timespan));
        // can't use sort_*_by_key: https://github.com/rust-lang/rust/issues/34162
        Ok(data)
    }
}

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
        _range: Option<Range<DateTimeUtc>>,
    ) -> Result<TimespanValue<Resolution, Value>, UpdateError> {
        let query = S::get_statement(&cx.blockscout_applied_migrations);
        let data = TimespanValue::<Resolution, Value>::find_by_statement(query)
            .one(cx.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?
            .ok_or_else(|| UpdateError::Internal("query returned nothing".into()))?;
        Ok(data)
    }
}
