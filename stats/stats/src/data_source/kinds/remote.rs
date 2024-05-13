use std::{marker::PhantomData, ops::RangeInclusive};

use chrono::NaiveDate;
use sea_orm::FromQueryResult;

use crate::{
    charts::db_interaction::chart_updaters::ChartBatchUpdater,
    data_source::{
        source_trait::DataSource,
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
    ) -> Result<Vec<DateValue>, UpdateError> {
        T::query_data(cx, range).await
    }
}

impl<T: ChartBatchUpdater> RemoteSource for T {
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
