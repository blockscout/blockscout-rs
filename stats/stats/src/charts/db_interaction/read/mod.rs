use sea_orm::DbErr;
use thiserror::Error;

use crate::ChartKey;

mod blockscout;
pub mod cached;
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
