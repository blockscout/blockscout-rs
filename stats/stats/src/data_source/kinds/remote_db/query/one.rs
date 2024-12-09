use std::marker::{PhantomData, Send};

use chrono::{DateTime, Utc};
use sea_orm::{FromQueryResult, Statement};

use crate::{
    data_source::{
        kinds::remote_db::RemoteQueryBehaviour,
        types::{BlockscoutMigrations, UpdateContext},
    },
    range::UniversalRange,
    types::TimespanValue,
    ChartError,
};

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
