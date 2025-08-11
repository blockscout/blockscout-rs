use std::ops::Range;

use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};
use thiserror::Error;

use crate::{
    ChartError, ChartKey,
    charts::db_interaction::read::multichain::get_min_date_multichain,
    data_source::{UpdateContext, kinds::remote_db::RemoteQueryBehaviour},
    range::UniversalRange,
};

mod blockscout;
pub mod cached;
mod local_db;
pub mod multichain;

pub use blockscout::*;
pub use local_db::*;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ReadError {
    #[error("database error {0}")]
    DB(#[from] DbErr),
    #[error("chart {0} not found")]
    ChartNotFound(ChartKey),
    #[error("exceeded limit on requested data points (~{0}); choose smaller time interval.")]
    IntervalTooLarge(u32),
}

pub struct QueryFullIndexerTimestampRange;

impl RemoteQueryBehaviour for QueryFullIndexerTimestampRange {
    type Output = Range<DateTime<Utc>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let min_date = get_min_date(cx.indexer_db, cx.is_multichain_mode).await;

        let start_timestamp = min_date.map_err(ChartError::IndexerDB)?.and_utc();
        Ok(start_timestamp..cx.time)
    }
}

pub async fn get_min_date(
    indexer_db: &DatabaseConnection,
    is_multichain: bool,
) -> Result<NaiveDateTime, DbErr> {
    if is_multichain {
        get_min_date_multichain(indexer_db).await
    } else {
        get_min_date_blockscout(indexer_db).await
    }
}
