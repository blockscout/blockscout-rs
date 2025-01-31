//! Remote database source.
//!
//! The main application - SQL queries from remote (=blockscout) database.
//!
//! ## Note
//!
//! [`RemoteDatabaseSource`] usually does not have any persistency and is only an
//! adapter for representing a remote DB as a `DataSource`.
//!
//! Since each [`RemoteQueryBehaviour::query_data`] performs (likely a heavy) database
//! query, it is undesireable to have this source present in more than one place.
//! Each (independent) appearance will likely result in requesting the same data
//! multiple times
//!
//! In this case, persistent behaviour of
//! [types in `local_db`](`crate::data_source::kinds::local_db`) is largely
//! helpful.

mod query;

use std::{
    future::Future,
    marker::{PhantomData, Send},
};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};

use crate::{
    data_source::{source::DataSource, types::UpdateContext},
    range::UniversalRange,
    ChartError,
};

pub use query::{
    PullAllWithAndSort, PullEachWith, PullOne, PullOne24hCached, PullOneValue, StatementForOne,
    StatementFromRange, StatementFromTimespan, StatementFromUpdateTime,
};

/// See [module-level documentation](self)
pub struct RemoteDatabaseSource<Q: RemoteQueryBehaviour>(PhantomData<Q>);

pub trait RemoteQueryBehaviour {
    type Output: Send;

    /// Retrieve chart data from remote storage.
    fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
    ) -> impl Future<Output = Result<Self::Output, ChartError>> + Send;
}

impl<Q: RemoteQueryBehaviour> DataSource for RemoteDatabaseSource<Q> {
    type MainDependencies = ();
    type ResolutionDependencies = ();
    type Output = Q::Output;
    // No local state => no race conditions expected
    fn mutex_id() -> Option<String> {
        None
    }

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &DateTime<Utc>,
    ) -> Result<(), DbErr> {
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<<Self as DataSource>::Output, ChartError> {
        let _interval = remote_fetch_timer.start_interval();
        Q::query_data(cx, range).await
    }

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), ChartError> {
        Ok(())
    }

    async fn set_next_update_from_itself(
        _db: &DatabaseConnection,
        _update_from: chrono::NaiveDate,
    ) -> Result<(), ChartError> {
        Ok(())
    }
}
