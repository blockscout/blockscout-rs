use crate::{DateValue, ReadError};
use chrono::{DateTime, Duration, Offset, TimeZone, Utc};
use entity::{charts, sea_orm_active_enums::ChartType};
use sea_orm::{prelude::*, sea_query, FromQueryResult, QuerySelect, Set};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("blockscout database error: {0}")]
    BlockscoutDB(DbErr),
    #[error("stats database error: {0}")]
    StatsDB(DbErr),
    #[error("chart {0} not found")]
    NotFound(String),
    #[error("date interval limit ({limit}) is exceeded; choose smaller time interval.")]
    IntervalLimitExceeded { limit: Duration },
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<ReadError> for UpdateError {
    fn from(read: ReadError) -> Self {
        match read {
            ReadError::DB(db) => UpdateError::StatsDB(db),
            ReadError::NotFound(err) => UpdateError::NotFound(err),
            ReadError::IntervalLimitExceeded(limit) => UpdateError::IntervalLimitExceeded { limit },
        }
    }
}

#[derive(Clone)]
pub struct ChartMetadata {
    pub last_update: DateTime<Utc>,
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
    fn name() -> &'static str;
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

#[derive(Debug, FromQueryResult)]
struct ChartID {
    id: i32,
}

pub async fn find_chart(db: &DatabaseConnection, name: &str) -> Result<Option<i32>, DbErr> {
    charts::Entity::find()
        .column(charts::Column::Id)
        .filter(charts::Column::Name.eq(name))
        .into_model::<ChartID>()
        .one(db)
        .await
        .map(|id| id.map(|id| id.id))
}

pub async fn create_chart<Tz: TimeZone>(
    db: &DatabaseConnection,
    name: String,
    chart_type: ChartType,
    creation_time: &DateTime<Tz>,
) -> Result<(), DbErr> {
    let id = find_chart(db, &name).await?;
    if id.is_some() {
        return Ok(());
    }
    charts::Entity::insert(charts::ActiveModel {
        name: Set(name),
        chart_type: Set(chart_type),
        created_at: Set(creation_time.with_timezone(&creation_time.offset().fix())),
        ..Default::default()
    })
    .on_conflict(
        sea_query::OnConflict::column(charts::Column::Name)
            .do_nothing()
            .to_owned(),
    )
    .exec(db)
    .await?;
    Ok(())
}
