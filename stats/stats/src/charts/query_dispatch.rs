use std::{fmt::Debug, future::Future, ops::Range, sync::Arc};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use stats_proto::blockscout::stats::v1::Point;

use crate::{
    data_source::{
        kinds::local_db::{
            parameter_traits::{CreateBehaviour, QueryBehaviour, UpdateBehaviour},
            LocalDbChartSource,
        },
        DataSource, UpdateContext,
    },
    exclusive_datetime_range_to_inclusive,
};

use super::{
    types::{ExtendedTimespanValue, Timespan, TimespanValue},
    ChartProperties, UpdateError,
};

/// Data query trait with unified data format (for external use)
pub trait QuerySerialized {
    /// Currently `Point` or `Vec<Point>`
    type Output: Send;

    /// Retrieve chart data from local storage.
    fn query_data<'a>(
        &self,
        cx: &UpdateContext<'a>,
        range: Option<Range<DateTime<Utc>>>,
        fill_missing_dates: bool,
    ) -> Box<dyn Future<Output = Result<Self::Output, UpdateError>> + Send + 'a>;
}

/// [`QuerySerialized`] but for dynamic dispatch
pub type QuerySerializedDyn<O> = Arc<Box<dyn QuerySerialized<Output = O> + Send + Sync>>;

pub enum ChartTypeSpecifics {
    Counter {
        query: QuerySerializedDyn<TimespanValue<NaiveDate, String>>,
    },
    Line {
        query: QuerySerializedDyn<Vec<Point>>,
    },
}

impl Debug for ChartTypeSpecifics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Counter { query: _ } => write!(f, "Counter"),
            Self::Line { query: _ } => write!(f, "Line"),
        }
    }
}

impl ChartTypeSpecifics {
    pub fn as_chart_type(&self) -> ChartType {
        match self {
            Self::Counter { query: _ } => ChartType::Counter,
            Self::Line { query: _ } => ChartType::Line,
        }
    }
}

impl Into<ChartTypeSpecifics> for QuerySerializedDyn<TimespanValue<NaiveDate, String>> {
    fn into(self) -> ChartTypeSpecifics {
        ChartTypeSpecifics::Counter { query: self }
    }
}

impl Into<ChartTypeSpecifics> for QuerySerializedDyn<Vec<Point>> {
    fn into(self) -> ChartTypeSpecifics {
        ChartTypeSpecifics::Line { query: self }
    }
}

pub trait SerializableQueryOutput {
    type Serialized;
    fn serialize(self) -> Self::Serialized;
}

impl<Resolution: Timespan> SerializableQueryOutput
    for Vec<ExtendedTimespanValue<Resolution, String>>
{
    type Serialized = Vec<Point>;

    fn serialize(self) -> Self::Serialized {
        serialize_line_points(self)
    }
}

impl SerializableQueryOutput for TimespanValue<NaiveDate, String> {
    type Serialized = TimespanValue<NaiveDate, String>;

    fn serialize(self) -> Self::Serialized {
        self
    }
}

impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps, QueryOutput> QuerySerialized
    for LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    Create: CreateBehaviour,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution>,
    Query: QueryBehaviour<Output = QueryOutput>,
    QueryOutput: SerializableQueryOutput,
    QueryOutput::Serialized: Send,
    ChartProps: ChartProperties,
{
    type Output = QueryOutput::Serialized;

    fn query_data<'a>(
        &self,
        cx: &UpdateContext<'a>,
        range: Option<Range<DateTime<Utc>>>,
        fill_missing_dates: bool,
    ) -> Box<dyn std::future::Future<Output = Result<Self::Output, UpdateError>> + Send + 'a> {
        let cx = cx.clone();
        Box::new(async move {
            let data = Query::query_data(&cx, range, fill_missing_dates).await?;
            Ok(data.serialize())
        })
    }
}

fn serialize_point<Resolution: Timespan>(
    point: ExtendedTimespanValue<Resolution, String>,
) -> Point {
    let time_range = exclusive_datetime_range_to_inclusive(point.timespan.into_time_range());
    let date_range = { time_range.start().date_naive()..=time_range.end().date_naive() };
    Point {
        date: date_range.start().to_string(),
        date_to: date_range.end().to_string(),
        value: point.value,
        is_approximate: point.is_approximate,
    }
}

pub fn serialize_line_points<Resolution: Timespan>(
    data: Vec<ExtendedTimespanValue<Resolution, String>>,
) -> Vec<Point> {
    data.into_iter().map(serialize_point).collect()
}
