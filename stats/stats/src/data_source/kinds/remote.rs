//! Remote data source.
//!
//! The main application - SQL queries from remote database.
//!
//! Usually used inside
//! [`RemoteChart`](`crate::data_source::kinds::updateable_chart::batch::remote::RemoteChart`)
//! data source.
//!
//! Note that since each `query_data` performs (likely a heavy)
//! query, it is undesireable to have this source present in
//! different places. For each such appearance, the same data
//! will be requested again.
//! In this case,
//! [`RemoteChart`](`crate::data_source::kinds::updateable_chart::batch::remote::RemoteChart`)
//! can be helpful to reuse the query results (by storing it locally).
use std::{marker::PhantomData, ops::RangeInclusive};

use blockscout_metrics_tools::AggregateTimer;
use sea_orm::{prelude::DateTimeUtc, FromQueryResult, Statement};

use crate::{
    data_source::{source::DataSource, types::UpdateContext},
    DateValue, UpdateError,
};

/// See [module-level documentation](self) for details.
pub trait RemoteSource {
    /// It is valid to have query results to be unsorted
    /// although it's not encouraged
    fn get_query(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement;

    fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<RangeInclusive<DateTimeUtc>>,
    ) -> impl std::future::Future<Output = Result<Vec<DateValue>, UpdateError>> + std::marker::Send
    {
        async move {
            let query = Self::get_query(range);
            let mut data = DateValue::find_by_statement(query)
                .all(cx.blockscout)
                .await
                .map_err(UpdateError::BlockscoutDB)?;
            // linear time for sorted sequences
            data.sort_unstable_by_key(|v| v.date);
            Ok(data)
        }
    }
}

/// Wrapper struct used for avoiding implementation conflicts.
///
/// See [module-level documentation](self) for details.
pub struct RemoteSourceWrapper<T: RemoteSource>(PhantomData<T>);

impl<T: RemoteSource> DataSource for RemoteSourceWrapper<T> {
    type PrimaryDependency = ();
    type SecondaryDependencies = ();
    type Output = Vec<DateValue>;
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
    ) -> Result<Vec<DateValue>, UpdateError> {
        let _interval = remote_fetch_timer.start_interval();
        T::query_data(cx, range).await
    }

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        Ok(())
    }
}
