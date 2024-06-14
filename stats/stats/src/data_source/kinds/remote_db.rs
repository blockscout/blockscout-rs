//! Remote database source.
//!
//! The main application - SQL queries from remote database.
//!
//! Usually used inside
//! [`CloneChart`](`crate::data_source::kinds::updateable_chart::clone::CloneChart`)
//! data source.
//!
//! Note that since each `query_data` performs (likely a heavy)
//! query, it is undesireable to have this source present in
//! different places. For each such appearance, the same data
//! will be requested again.
//! In this case,
//! [`CloneChart`](`crate::data_source::kinds::updateable_chart::clone::CloneChart`)
//! can be helpful to reuse the query results (by storing it locally).

use std::{
    future::Future,
    marker::{PhantomData, Send},
    ops::RangeInclusive,
};

use blockscout_metrics_tools::AggregateTimer;
use sea_orm::{prelude::DateTimeUtc, Statement};

use crate::{
    charts::chart::Point,
    data_source::{source::DataSource, types::UpdateContext},
    UpdateError,
};

/// todo: docs
pub struct RemoteDatabaseSource<Q: QueryBehaviour>(PhantomData<Q>);

pub trait QueryBehaviour {
    /// Currently `P` or `Vec<P>`, where `P` is some type representing a data point
    type Output: Send;

    fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<RangeInclusive<DateTimeUtc>>,
    ) -> impl Future<Output = Result<Self::Output, UpdateError>> + Send;
}

impl<Q: QueryBehaviour> DataSource for RemoteDatabaseSource<Q> {
    type MainDependencies = ();
    type ResolutionDependencies = ();
    type Output = Q::Output;
    // No local state => no race conditions expected
    const MUTEX_ID: Option<&'static str> = None;

    async fn init_itself(
        _db: &::sea_orm::DatabaseConnection,
        _init_time: &::chrono::DateTime<chrono::Utc>,
    ) -> Result<(), ::sea_orm::DbErr> {
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<RangeInclusive<DateTimeUtc>>,
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
    fn get_statement(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement;
}

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
        range: Option<RangeInclusive<DateTimeUtc>>,
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
        _range: Option<RangeInclusive<DateTimeUtc>>,
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
