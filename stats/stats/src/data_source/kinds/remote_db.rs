//! Remote database source.
//!
//! The main application - SQL queries from remote (=blockscout) database.
//!
//! ## Details
//!
//! This source does not have any persistency and is only an adapter for representing
//! a remote DB as a `DataSource`.
//!
//! Since each [`QueryBehaviour::query_data`] performs (likely a heavy) database
//! query, it is undesireable to have this source present in more than one place.
//! Each (independent) appearance will likely result in requesting the same data
//! multiple times
//!
//! In this case, persistent behaviour of
//! [types in `local_db`](`crate::data_source::kinds::local_db`) is largely
//! helpful.

use std::{
    future::Future,
    marker::{PhantomData, Send},
    ops::Range,
};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr, Statement};

use crate::{
    charts::Point,
    data_source::{source::DataSource, types::UpdateContext},
    UpdateError,
};

/// See [module-level documentation](self)
pub struct RemoteDatabaseSource<Q: QueryBehaviour>(PhantomData<Q>);

pub trait QueryBehaviour {
    type Output: Send;

    fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
    ) -> impl Future<Output = Result<Self::Output, UpdateError>> + Send;
}

impl<Q: QueryBehaviour> DataSource for RemoteDatabaseSource<Q> {
    type MainDependencies = ();
    type ResolutionDependencies = ();
    type Output = Q::Output;
    // No local state => no race conditions expected
    const MUTEX_ID: Option<&'static str> = None;

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &DateTime<Utc>,
    ) -> Result<(), DbErr> {
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<<Self as DataSource>::Output, UpdateError> {
        let _interval = remote_fetch_timer.start_interval();
        Q::query_data(cx, range).await
    }

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        Ok(())
    }
}

pub trait StatementFromRange {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement;
}

/// Pull data from remote (blockscout) db according to statement
/// `S` and sort it by date.
///
/// `P` - Type of point to retrieve within query.
/// `DateValueString` can be used to avoid parsing the values,
/// but `DateValueDecimal` or other types can be useful sometimes.
pub struct PullAllWithAndSort<S: StatementFromRange, P: Point>(PhantomData<(S, P)>);

impl<S, P> QueryBehaviour for PullAllWithAndSort<S, P>
where
    S: StatementFromRange,
    P: Point,
{
    type Output = Vec<P>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
    ) -> Result<Vec<P>, UpdateError> {
        let query = S::get_statement(range);
        let mut data = P::find_by_statement(query)
            .all(cx.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        // linear time for sorted sequences
        data.sort_unstable_by(|a, b| a.get_parts().0.cmp(b.get_parts().0));
        // can't use sort_*_by_key: https://github.com/rust-lang/rust/issues/34162
        Ok(data)
    }
}

pub trait StatementForOne {
    fn get_statement() -> Statement;
}

/// Get a single record from remote (blockscout) DB using statement
/// `S`.
///
/// `P` - Type of point to retrieve within query.
/// `DateValueString` can be used to avoid parsing the values,
/// but `DateValueDecimal` or other types can be useful sometimes.
pub struct PullOne<S: StatementForOne, P: Point>(PhantomData<(S, P)>);

impl<S, P> QueryBehaviour for PullOne<S, P>
where
    S: StatementForOne,
    P: Point,
{
    type Output = P;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: Option<Range<DateTimeUtc>>,
    ) -> Result<P, UpdateError> {
        let query = S::get_statement();
        let data = P::find_by_statement(query)
            .one(cx.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?
            .ok_or_else(|| UpdateError::Internal("query returned nothing".into()))?;
        Ok(data)
    }
}
