use crate::{DateValue, ReadError};
use chrono::{DateTime, Duration, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("blockscout database error: {0}")]
    BlockscoutDB(DbErr),
    #[error("stats database error: {0}")]
    StatsDB(DbErr),
    #[error("chart {0} not found")]
    ChartNotFound(String),
    #[error("date interval limit ({limit}) is exceeded; choose smaller time interval.")]
    IntervalLimitExceeded { limit: Duration },
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<ReadError> for UpdateError {
    fn from(read: ReadError) -> Self {
        match read {
            ReadError::DB(db) => UpdateError::StatsDB(db),
            ReadError::ChartNotFound(err) => UpdateError::ChartNotFound(err),
            ReadError::IntervalLimitExceeded(limit) => UpdateError::IntervalLimitExceeded { limit },
        }
    }
}

#[derive(Clone)]
pub struct ChartMetadata {
    pub id: i32,
    pub created_at: DateTime<Utc>,
    pub last_updated_at: Option<DateTime<Utc>>,
}

pub struct ChartData {
    pub metadata: ChartMetadata,
    pub values: Vec<DateValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingDatePolicy {
    FillZero,
    FillPrevious,
}

#[portrait::make(import(
    crate::charts::chart::MissingDatePolicy,
    entity::sea_orm_active_enums::ChartType,
))]
pub trait Chart: Sync {
    /// Must be unique
    const NAME: &'static str;
    fn chart_type() -> ChartType;
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillZero
    }
    fn relevant_or_zero() -> bool {
        false
    }
    /// Number of last values that are considered approximate.
    /// (ordered by time)
    ///
    /// E.g. how many end values should be recalculated on (kinda)
    /// lazy update (where `get_last_row` is retrieved successfully).
    /// Also controls marking points as approximate when returning data.
    ///
    /// ## Value
    ///
    /// Usually set to 1 for line charts. Currently they have resolution of
    /// 1 day. Also, data for portion of the (last) day has to be recalculated
    /// on the next day.
    ///
    /// I.e. for number of blocks per day, stats for current day (0) are
    /// not complete because blocks will be produced till the end of the day.
    ///    |===|=  |
    /// day -1   0
    fn approximate_trailing_points() -> u64 {
        if Self::chart_type() == ChartType::Counter {
            // there's only one value in counter
            0
        } else {
            1
        }
    }
}

/// Dynamic version of trait `Chart`.
///
/// Helpful when need a unified type for different charts
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartDynamic {
    pub name: String,
    pub chart_type: ChartType,
    pub missing_date_policy: MissingDatePolicy,
    pub relevant_or_zero: bool,
    pub approximate_trailing_points: u64,
}

impl ChartDynamic {
    pub fn construct_from_chart<T: Chart>() -> Self {
        Self {
            name: T::NAME.to_owned(),
            chart_type: T::chart_type(),
            missing_date_policy: T::missing_date_policy(),
            relevant_or_zero: T::relevant_or_zero(),
            approximate_trailing_points: T::approximate_trailing_points(),
        }
    }
}
