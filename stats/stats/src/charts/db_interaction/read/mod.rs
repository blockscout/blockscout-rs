use std::ops::Range;

use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};
use thiserror::Error;

use crate::{
    ChartError, ChartKey, Mode, charts::db_interaction::read::{
        interchain::get_min_date_interchain,
        multichain::get_min_date_multichain,
    }, data_source::{UpdateContext, kinds::remote_db::RemoteQueryBehaviour}, range::UniversalRange
};

mod blockscout;
pub mod cached;
mod local_db;
pub mod interchain;
pub mod multichain;
pub mod zetachain_cctx;

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
        let min_date = get_min_date(cx.indexer_db, cx.mode).await;

        let start_timestamp = min_date.map_err(ChartError::IndexerDB)?.and_utc();
        Ok(start_timestamp..cx.time)
    }
}

pub async fn get_min_date(
    indexer_db: &DatabaseConnection,
    mode: crate::Mode,
) -> Result<NaiveDateTime, DbErr> {
    match mode {
        Mode::Interchain => get_min_date_interchain(indexer_db).await,
        Mode::Aggregator => get_min_date_multichain(indexer_db).await,
        _ => get_min_date_blockscout(indexer_db).await,
    }
}
