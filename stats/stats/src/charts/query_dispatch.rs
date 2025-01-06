use std::{fmt::Debug, future::Future, pin::Pin, sync::Arc};

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
    range::{exclusive_range_to_inclusive, UniversalRange},
    RequestedPointsLimit,
};

use super::{
    types::{ExtendedTimespanValue, Timespan, TimespanValue},
    ChartError, ChartProperties,
};

/// Data query trait with unified data format (for external use)
pub trait QuerySerialized {
    /// Currently `Point` or `Vec<Point>`
    type Output: Send;

    /// `new` function that is created solely for the purposes of
    /// dynamic dispatch (see where it's used).
    fn new_for_dynamic_dispatch() -> Self
    where
        Self: Sized;

    /// Retrieve chart data from local storage.
    fn query_data<'a>(
        &self,
        cx: &'a UpdateContext<'a>,
        range: UniversalRange<DateTime<Utc>>,
        points_limit: Option<RequestedPointsLimit>,
        fill_missing_dates: bool,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, ChartError>> + Send + 'a>>;

    /// Retrieve chart data from local storage.
    fn query_data_static<'a>(
        cx: &'a UpdateContext<'a>,
        range: UniversalRange<DateTime<Utc>>,
        points_limit: Option<RequestedPointsLimit>,
        fill_missing_dates: bool,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, ChartError>> + Send + 'a>>
    where
        Self: Sized,
    {
        Self::new_for_dynamic_dispatch().query_data(cx, range, points_limit, fill_missing_dates)
    }
}

/// [`QuerySerialized`] but for dynamic dispatch
pub type QuerySerializedDyn<O> = Arc<Box<dyn QuerySerialized<Output = O> + Send + Sync>>;

pub type CounterHandle = QuerySerializedDyn<TimespanValue<NaiveDate, String>>;
pub type LineHandle = QuerySerializedDyn<Vec<Point>>;

#[derive(Clone)]
pub enum ChartTypeSpecifics {
    Counter { query: CounterHandle },
    Line { query: LineHandle },
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

    pub fn into_counter_handle(self) -> Option<CounterHandle> {
        match self {
            Self::Counter { query } => Some(query),
            _ => None,
        }
    }

    pub fn into_line_handle(self) -> Option<LineHandle> {
        match self {
            Self::Line { query } => Some(query),
            _ => None,
        }
    }
}

impl From<CounterHandle> for ChartTypeSpecifics {
    fn from(val: CounterHandle) -> Self {
        ChartTypeSpecifics::Counter { query: val }
    }
}

impl From<LineHandle> for ChartTypeSpecifics {
    fn from(val: LineHandle) -> Self {
        ChartTypeSpecifics::Line { query: val }
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
    MainDep: DataSource + Sync,
    ResolutionDep: DataSource + Sync,
    Create: CreateBehaviour + Sync,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution> + Sync,
    Query: QueryBehaviour<Output = QueryOutput> + Sync,
    QueryOutput: SerializableQueryOutput,
    QueryOutput::Serialized: Send,
    ChartProps: ChartProperties,
    ChartProps::Resolution: Ord + Clone + Debug,
{
    type Output = QueryOutput::Serialized;

    fn query_data<'a>(
        &self,
        cx: &'a UpdateContext<'a>,
        range: UniversalRange<DateTime<Utc>>,
        points_limit: Option<RequestedPointsLimit>,
        fill_missing_dates: bool,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Self::Output, ChartError>> + Send + 'a>>
    {
        let cx = cx.clone();
        Box::pin(async move {
            let data = Query::query_data(&cx, range, points_limit, fill_missing_dates).await?;
            Ok(data.serialize())
        })
    }

    fn new_for_dynamic_dispatch() -> Self
    where
        Self: Sized,
    {
        Self(std::marker::PhantomData)
    }
}

fn serialize_point<Resolution: Timespan>(
    point: ExtendedTimespanValue<Resolution, String>,
) -> Point {
    let time_range = exclusive_range_to_inclusive(point.timespan.into_time_range());
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
