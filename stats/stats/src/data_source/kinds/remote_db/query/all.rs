use std::{
    marker::{PhantomData, Send},
    ops::Range,
};

use chrono::{DateTime, Utc};
use sea_orm::{FromQueryResult, Statement};

use crate::{
    charts::db_interaction::read::cached::find_all_cached,
    data_source::{
        kinds::remote_db::RemoteQueryBehaviour,
        types::{BlockscoutMigrations, Cacheable, UpdateContext},
    },
    range::{data_source_query_range_to_db_statement_range, UniversalRange},
    types::{TimespanTrait, TimespanValue},
    ChartError,
};

pub trait StatementFromRange {
    /// `completed_migrations` can be used for selecting more optimal query
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement;
}

/// Pull data from remote (blockscout) db according to statement
/// `S` and sort it by date.
///
/// `P` - Type of point to retrieve within query.
/// `DateValue<String>` can be used to avoid parsing the values,
/// but `DateValue<Decimal>` or other types can be useful sometimes.
pub struct PullAllWithAndSort<S, Resolution, Value, AllRangeSource>(
    PhantomData<(S, Resolution, Value, AllRangeSource)>,
)
where
    S: StatementFromRange,
    Resolution: Ord + Send,
    Value: Send,
    TimespanValue<Resolution, Value>: FromQueryResult,
    AllRangeSource: RemoteQueryBehaviour<Output = Range<DateTime<Utc>>>;

impl<S, Resolution, Value, AllRangeSource> RemoteQueryBehaviour
    for PullAllWithAndSort<S, Resolution, Value, AllRangeSource>
where
    S: StatementFromRange,
    Resolution: Ord + Send,
    Value: Send,
    TimespanValue<Resolution, Value>: FromQueryResult,
    AllRangeSource: RemoteQueryBehaviour<Output = Range<DateTime<Utc>>>,
{
    type Output = Vec<TimespanValue<Resolution, Value>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Vec<TimespanValue<Resolution, Value>>, ChartError> {
        // to not overcomplicate the queries
        let query_range =
            data_source_query_range_to_db_statement_range::<AllRangeSource>(cx, range).await?;
        let query = S::get_statement(query_range, &cx.blockscout_applied_migrations);
        let mut data = TimespanValue::<Resolution, Value>::find_by_statement(query)
            .all(cx.blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?;
        // linear time for sorted sequences
        data.sort_unstable_by(|a, b| a.timespan.cmp(&b.timespan));
        // can't use sort_*_by_key: https://github.com/rust-lang/rust/issues/34162
        Ok(data)
    }
}
/// Pull data from remote (blockscout) db according to statement
/// `S`, sort it by date and cache the result (within one update).
///
/// See [`PullAllWithAndSort`] for more info.
pub struct PullAllWithAndSortCached<S, Point, AllRangeSource>(
    PhantomData<(S, Point, AllRangeSource)>,
)
where
    S: StatementFromRange,
    Point: FromQueryResult + TimespanTrait + Clone + Send,
    Point::Timespan: Ord,
    Vec<Point>: Cacheable,
    AllRangeSource: RemoteQueryBehaviour<Output = Range<DateTime<Utc>>>;

impl<S, Point, AllRangeSource> RemoteQueryBehaviour
    for PullAllWithAndSortCached<S, Point, AllRangeSource>
where
    S: StatementFromRange,
    Point: FromQueryResult + TimespanTrait + Clone + Send,
    Point::Timespan: Ord,
    Vec<Point>: Cacheable,
    AllRangeSource: RemoteQueryBehaviour<Output = Range<DateTime<Utc>>>,
{
    type Output = Vec<Point>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Vec<Point>, ChartError> {
        // to not overcomplicate the queries
        let query_range =
            data_source_query_range_to_db_statement_range::<AllRangeSource>(cx, range).await?;
        let query = S::get_statement(query_range, &cx.blockscout_applied_migrations);
        let data = find_all_cached(cx, query).await?;
        Ok(data)
    }
}
