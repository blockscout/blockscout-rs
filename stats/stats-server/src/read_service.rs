use std::{clone::Clone, cmp::Ord, collections::BTreeMap, fmt::Debug, str::FromStr, sync::Arc};

use crate::{
    config::types,
    runtime_setup::{EnabledChartEntry, RuntimeSetup},
    serializers::serialize_line_points,
    settings::LimitsSettings,
};

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use proto_v1::stats_service_server::StatsService;
use sea_orm::{DatabaseConnection, DbErr};
use stats::{
    entity::sea_orm_active_enums::ChartType,
    types::{
        timespans::{Month, Week, Year},
        Timespan,
    },
    ApproxUnsignedDiff, MissingDatePolicy, ReadError, RequestedPointsLimit, ResolutionKind,
};
use stats_proto::blockscout::stats::v1::{self as proto_v1, Point};
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct ReadService {
    db: Arc<DatabaseConnection>,
    charts: Arc<RuntimeSetup>,
    limits: ReadLimits,
}

impl ReadService {
    pub async fn new(
        db: Arc<DatabaseConnection>,
        charts: Arc<RuntimeSetup>,
        limits: ReadLimits,
    ) -> Result<Self, DbErr> {
        Ok(Self { db, charts, limits })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadLimits {
    /// See [`LimitsSettings::requested_points_limit`]
    pub requested_points_limit: RequestedPointsLimit,
}

impl From<LimitsSettings> for ReadLimits {
    fn from(value: LimitsSettings) -> Self {
        Self {
            requested_points_limit: RequestedPointsLimit::from_points(value.requested_points_limit),
        }
    }
}

fn map_read_error(err: ReadError) -> Status {
    match &err {
        ReadError::ChartNotFound(_) => Status::not_found(err.to_string()),
        ReadError::IntervalTooLarge(_) => Status::invalid_argument(err.to_string()),
        _ => {
            tracing::error!(err = ?err, "internal read error");
            Status::internal(err.to_string())
        }
    }
}

/// Add chart information to each chart id in layout
///
/// Returns `None` if info were not found for some chart.
fn add_chart_info_to_layout(
    layout: &[types::LineChartCategory],
    chart_info: &BTreeMap<String, EnabledChartEntry>,
) -> Vec<proto_v1::LineChartSection> {
    layout
        .iter()
        .map(|cat| cat.intersect_info(chart_info))
        .collect()
}

fn convert_resolution(input: proto_v1::Resolution) -> ResolutionKind {
    match input {
        proto_v1::Resolution::Unspecified | proto_v1::Resolution::Day => ResolutionKind::Day,
        proto_v1::Resolution::Week => ResolutionKind::Week,
        proto_v1::Resolution::Month => ResolutionKind::Month,
        proto_v1::Resolution::Year => ResolutionKind::Year,
    }
}

async fn get_serialized_line_chart_data<Resolution>(
    db: &DatabaseConnection,
    chart_name: String,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    points_limit: Option<RequestedPointsLimit>,
    policy: MissingDatePolicy,
    mark_approx: u64,
) -> Result<Vec<Point>, ReadError>
where
    Resolution: Timespan + ApproxUnsignedDiff + Clone + Ord + Debug,
{
    let from = from.map(|f| Resolution::from_date(f));
    let to = to.map(|t| Resolution::from_date(t));
    let data = stats::get_line_chart_data::<Resolution>(
        db,
        &chart_name,
        from,
        to,
        points_limit,
        policy,
        true,
        mark_approx,
    )
    .await?;
    Ok(serialize_line_points(data))
}

/// enum dispatch for `get_serialized_line_chart_data`
#[allow(clippy::too_many_arguments)]
async fn get_serialized_line_chart_data_resolution_dispatch(
    db: &DatabaseConnection,
    chart_name: String,
    resolution: ResolutionKind,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    points_limit: Option<RequestedPointsLimit>,
    policy: MissingDatePolicy,
    mark_approx: u64,
) -> Result<Vec<Point>, ReadError> {
    match resolution {
        ResolutionKind::Day => {
            get_serialized_line_chart_data::<NaiveDate>(
                db,
                chart_name,
                from,
                to,
                points_limit,
                policy,
                mark_approx,
            )
            .await
        }
        ResolutionKind::Week => {
            get_serialized_line_chart_data::<Week>(
                db,
                chart_name,
                from,
                to,
                points_limit,
                policy,
                mark_approx,
            )
            .await
        }
        ResolutionKind::Month => {
            get_serialized_line_chart_data::<Month>(
                db,
                chart_name,
                from,
                to,
                points_limit,
                policy,
                mark_approx,
            )
            .await
        }
        ResolutionKind::Year => {
            get_serialized_line_chart_data::<Year>(
                db,
                chart_name,
                from,
                to,
                points_limit,
                policy,
                mark_approx,
            )
            .await
        }
    }
}

#[async_trait]
impl StatsService for ReadService {
    async fn get_counters(
        &self,
        _request: Request<proto_v1::GetCountersRequest>,
    ) -> Result<Response<proto_v1::Counters>, Status> {
        let mut data = stats::get_raw_counters(&self.db)
            .await
            .map_err(map_read_error)?;

        let counters = self
            .charts
            .charts_info
            .iter()
            .filter(|(_, chart)| {
                chart
                    .enabled_resolutions
                    .iter()
                    .all(|(_, static_info)| static_info.chart_type == ChartType::Counter)
            })
            .filter_map(|(name, counter)| {
                data.remove(name).and_then(|point| {
                    // resolutions other than day are currently not supported
                    // for counters
                    let Some(static_info) = counter.enabled_resolutions.get(&ResolutionKind::Day)
                    else {
                        tracing::warn!(
                            "No 'day' resolution enabled for counter {}, skipping its value",
                            name
                        );
                        return None;
                    };
                    let point = if static_info.missing_date_policy == MissingDatePolicy::FillZero {
                        point.relevant_or_zero(Utc::now().date_naive())
                    } else {
                        point
                    };
                    Some(proto_v1::Counter {
                        id: static_info.name.clone(),
                        value: point.value,
                        title: counter.settings.title.clone(),
                        description: counter.settings.description.clone(),
                        units: counter.settings.units.clone(),
                    })
                })
            })
            .collect();
        let counters = proto_v1::Counters { counters };
        Ok(Response::new(counters))
    }

    async fn get_line_chart(
        &self,
        request: Request<proto_v1::GetLineChartRequest>,
    ) -> Result<Response<proto_v1::LineChart>, Status> {
        let request = request.into_inner();
        let resolution = convert_resolution(request.resolution());
        let chart_name = request.name;
        let chart_entry = self.charts.charts_info.get(&chart_name).ok_or_else(|| {
            Status::not_found(format!("chart with name '{}' was not found", chart_name))
        })?;
        let resolution_info = chart_entry
            .enabled_resolutions
            .get(&resolution)
            .filter(|static_info| static_info.chart_type == ChartType::Line)
            .ok_or_else(|| {
                Status::not_found(format!(
                    "resolution '{}' for chart '{}' was not found",
                    String::from(resolution),
                    chart_name,
                ))
            })?;

        let from = request
            .from
            .and_then(|date| NaiveDate::from_str(&date).ok());
        let to = request.to.and_then(|date| NaiveDate::from_str(&date).ok());
        let policy = resolution_info.missing_date_policy;
        let mark_approx = resolution_info.approximate_trailing_points;
        let points_limit = Some(self.limits.requested_points_limit);
        let serialized_chart = get_serialized_line_chart_data_resolution_dispatch(
            &self.db,
            chart_name.clone(),
            resolution,
            from,
            to,
            points_limit,
            policy,
            mark_approx,
        )
        .await
        .map_err(map_read_error)?;
        Ok(Response::new(proto_v1::LineChart {
            chart: serialized_chart,
            info: Some(chart_entry.build_proto_line_chart_info(chart_name)),
        }))
    }

    async fn get_line_charts(
        &self,
        _request: Request<proto_v1::GetLineChartsRequest>,
    ) -> Result<Response<proto_v1::LineCharts>, Status> {
        let sections = add_chart_info_to_layout(
            self.charts.lines_layout.as_slice(),
            &self.charts.charts_info,
        );

        Ok(Response::new(proto_v1::LineCharts { sections }))
    }
}
