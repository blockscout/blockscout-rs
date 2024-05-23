use std::{marker::PhantomData, ops::RangeInclusive};

use blockscout_metrics_tools::AggregateTimer;
use chrono::NaiveDate;
use sea_orm::FromQueryResult;

use crate::{
    charts::db_interaction::chart_updaters::RemoteBatchQuery,
    data_source::{source::DataSource, types::UpdateContext},
    DateValue, UpdateError,
};

pub trait RemoteSource {
    fn query_data(
        cx: &UpdateContext<'_>,
        range: RangeInclusive<NaiveDate>,
    ) -> impl std::future::Future<Output = Result<Vec<DateValue>, UpdateError>> + std::marker::Send;
}

/// Wrapper struct used for avoiding implementation conflicts.
///
/// Note that since each `query_data` performs (likely a heavy)
/// query, it is undesireable to have this source present in
/// different places. In this case,
/// [`crate::data_source::kinds::chart::RemoteChart`]
/// can be helpful to reuse the query results.
pub struct RemoteSourceWrapper<T: RemoteSource>(PhantomData<T>);

impl<T: RemoteSource> DataSource for RemoteSourceWrapper<T> {
    type PrimaryDependency = ();
    type SecondaryDependencies = ();
    type Output = Vec<DateValue>;
    // No local state => no race conditions expected
    const MUTEX_ID: Option<&'static str> = None;

    fn init_itself(
        _db: &::sea_orm::DatabaseConnection,
        _init_time: &::chrono::DateTime<chrono::Utc>,
    ) -> impl ::std::future::Future<Output = Result<(), ::sea_orm::DbErr>> + Send {
        async move { Ok(()) }
    }

    async fn update_from_remote(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: RangeInclusive<NaiveDate>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let _interval = remote_fetch_timer.start_interval();
        T::query_data(cx, range).await
    }
}

impl<T: RemoteBatchQuery> RemoteSource for T {
    async fn query_data(
        cx: &UpdateContext<'_>,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let query = T::get_query(*range.start(), *range.end());
        DateValue::find_by_statement(query)
            .all(cx.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)
    }
}
