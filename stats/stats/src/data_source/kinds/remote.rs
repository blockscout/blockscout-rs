use std::{marker::PhantomData, ops::RangeInclusive};

use blockscout_metrics_tools::AggregateTimer;
use chrono::NaiveDate;
use sea_orm::FromQueryResult;

use crate::{
    charts::db_interaction::chart_updaters::RemoteBatchQuery,
    data_source::{
        source::DataSource,
        types::{UpdateContext, UpdateParameters},
    },
    DateValue, UpdateError,
};

pub trait RemoteSource {
    async fn query_data(
        cx: &UpdateContext<UpdateParameters<'_>>,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<Vec<DateValue>, UpdateError>;
}

/// Wrapper struct used for avoiding implementation conflicts
pub struct RemoteSourceWrapper<T: RemoteSource>(PhantomData<T>);

impl<T: RemoteSource> DataSource for RemoteSourceWrapper<T> {
    type PrimaryDependency = ();
    type SecondaryDependencies = ();
    type Output = Vec<DateValue>;

    fn init_itself(
        _db: &::sea_orm::DatabaseConnection,
        _init_time: &::chrono::DateTime<chrono::Utc>,
    ) -> impl ::std::future::Future<Output = Result<(), ::sea_orm::DbErr>> + Send {
        async move { Ok(()) }
    }

    async fn update_from_remote(
        _cx: &UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError> {
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<UpdateParameters<'_>>,
        range: RangeInclusive<NaiveDate>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let _interval = remote_fetch_timer.start_interval();
        T::query_data(cx, range).await
    }
}

impl<T: RemoteBatchQuery> RemoteSource for T {
    async fn query_data(
        cx: &UpdateContext<UpdateParameters<'_>>,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let query = T::get_query(*range.start(), *range.end());
        DateValue::find_by_statement(query)
            .all(cx.user_context.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)
    }
}
