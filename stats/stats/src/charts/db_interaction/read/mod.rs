// SPDX-License-Identifier: LicenseRef-Blockscout

use std::ops::Range;

use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::DbErr;
use thiserror::Error;

use crate::{
    ChartError, ChartKey, Mode,
    charts::db_interaction::read::{
        interchain::get_min_date_interchain, multichain::get_min_date_multichain,
    },
    data_source::{UpdateContext, kinds::remote_db::RemoteQueryBehaviour},
    range::UniversalRange,
};

mod blockscout;
pub mod cached;
pub mod interchain;
mod local_db;
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
        let min_date = get_min_date(cx).await;

        let start_timestamp = min_date.map_err(ChartError::IndexerDB)?.and_utc();
        Ok(start_timestamp..cx.time)
    }
}

/// The earliest date the indexer has data for — the floor every batched backfill
/// starts from.
///
/// Takes the whole [`UpdateContext`] rather than a connection and a [`Mode`],
/// because in `Interchain` mode the floor is filter-dependent: see
/// [`get_min_date_interchain`].
pub async fn get_min_date(cx: &UpdateContext<'_>) -> Result<NaiveDateTime, DbErr> {
    match cx.mode {
        Mode::Interchain => get_min_date_interchain(cx.indexer_db, &cx.interchain_filter).await,
        Mode::MultichainAggregator => get_min_date_multichain(cx.indexer_db).await,
        Mode::Blockscout | Mode::Zetachain => get_min_date_blockscout(cx.indexer_db).await,
    }
}
