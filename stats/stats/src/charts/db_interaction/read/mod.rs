use std::ops::Range;

use chrono::{DateTime, Utc};
use sea_orm::DbErr;
use thiserror::Error;

use crate::{
    data_source::{kinds::remote_db::RemoteQueryBehaviour, UpdateContext},
    range::UniversalRange,
    ChartError, ChartKey,
};

mod blockscout;
pub mod cached;
mod local_db;

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

pub struct QueryAllBlockTimestampRange;

impl RemoteQueryBehaviour for QueryAllBlockTimestampRange {
    type Output = Range<DateTime<Utc>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let start_timestamp = get_min_date_blockscout(cx.blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?
            .and_utc();
        Ok(start_timestamp..cx.time)
    }
}
