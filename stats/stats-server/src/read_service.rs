use std::{clone::Clone, collections::BTreeMap, fmt::Debug, str::FromStr, sync::Arc};

use crate::{
    config::{
        layout::sorted_items_according_to_layout,
        types::{self, EnabledChartSettings},
    },
    runtime_setup::{EnabledChartEntry, RuntimeSetup},
    settings::LimitsSettings,
};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use futures::{stream::FuturesOrdered, StreamExt};
use proto_v1::stats_service_server::StatsService;
use sea_orm::{DatabaseConnection, DbErr};
use stats::{
    data_source::{types::BlockscoutMigrations, UpdateContext, UpdateParameters},
    query_dispatch::{CounterHandle, LineHandle, QuerySerializedDyn},
    range::UniversalRange,
    types::Timespan,
    utils::day_start,
    RequestedPointsLimit, ResolutionKind, UpdateError,
};
use stats_proto::blockscout::stats::v1 as proto_v1;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct ReadService {
    db: Arc<DatabaseConnection>,
    blockscout: Arc<DatabaseConnection>,
    charts: Arc<RuntimeSetup>,
    limits: ReadLimits,
}

impl ReadService {
    pub async fn new(
        db: Arc<DatabaseConnection>,
        blockscout: Arc<DatabaseConnection>,
        charts: Arc<RuntimeSetup>,
        limits: ReadLimits,
    ) -> Result<Self, DbErr> {
        Ok(Self {
            db,
            blockscout,
            charts,
            limits,
        })
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

fn map_update_error(err: UpdateError) -> Status {
    match &err {
        UpdateError::ChartNotFound(_) => Status::not_found(err.to_string()),
        UpdateError::IntervalTooLarge { limit: _ } => Status::invalid_argument(err.to_string()),
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
    layout: Vec<types::LineChartCategory>,
    chart_info: &BTreeMap<String, EnabledChartEntry>,
) -> Vec<proto_v1::LineChartSection> {
    layout
        .into_iter()
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

impl ReadService {
    async fn query_with_handle<Data: Send>(
        &self,
        query_handle: QuerySerializedDyn<Data>,
        range: UniversalRange<DateTime<Utc>>,
        points_limit: Option<RequestedPointsLimit>,
        query_time: DateTime<Utc>,
    ) -> Result<Data, UpdateError> {
        let migrations = BlockscoutMigrations::query_from_db(&self.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let context = UpdateContext::from_params_now_or_override(UpdateParameters {
            db: &self.db,
            blockscout: &self.blockscout,
            blockscout_applied_migrations: migrations,
            update_time_override: Some(query_time),
            force_full: false,
        });
        query_handle
            .query_data(&context, range, points_limit, true)
            .await
    }

    async fn query_counter_with_handle(
        &self,
        name: String,
        settings: EnabledChartSettings,
        query_handle: CounterHandle,
        query_time: DateTime<Utc>,
    ) -> anyhow::Result<proto_v1::Counter> {
        let point = self
            .query_with_handle(query_handle, UniversalRange::full(), None, query_time)
            .await?;
        Ok(proto_v1::Counter {
            id: name.to_string(),
            value: point.value,
            title: settings.title,
            description: settings.description,
            units: settings.units,
        })
    }

    async fn query_line_chart_with_handle(
        &self,
        name: String,
        chart_entry: &EnabledChartEntry,
        query_handle: LineHandle,
        range: UniversalRange<DateTime<Utc>>,
        points_limit: Option<RequestedPointsLimit>,
        query_time: DateTime<Utc>,
    ) -> Result<proto_v1::LineChart, UpdateError> {
        let data = self
            .query_with_handle(query_handle, range, points_limit, query_time)
            .await?;
        Ok(proto_v1::LineChart {
            chart: data,
            info: Some(chart_entry.build_proto_line_chart_info(name.to_string())),
        })
    }
}

#[async_trait]
impl StatsService for ReadService {
    async fn get_counters(
        &self,
        _request: Request<proto_v1::GetCountersRequest>,
    ) -> Result<Response<proto_v1::Counters>, Status> {
        let now = Utc::now();
        let counters_futures: FuturesOrdered<_> = self
            .charts
            .charts_info
            .iter()
            .filter_map(|(name, counter)| {
                // resolutions other than day are currently not supported
                // for counters
                let Some(enabled_resolution) = counter.resolutions.get(&ResolutionKind::Day) else {
                    tracing::warn!(
                        "No 'day' resolution enabled for counter {}, skipping its value",
                        name
                    );
                    return None;
                };
                let query_handle = enabled_resolution
                    .type_specifics
                    .clone()
                    .into_counter_handle()?;
                Some(self.query_counter_with_handle(
                    name.clone(),
                    counter.settings.clone(),
                    query_handle,
                    now,
                ))
            })
            .collect();
        let counters: Vec<proto_v1::Counter> = counters_futures
            .filter_map(|query_result| async move {
                match query_result {
                    Ok(c) => Some(c),
                    Err(e) => {
                        tracing::error!("Failed to query counter: {:?}", e);
                        None
                    }
                }
            })
            .collect()
            .await;
        let counters_sorted =
            sorted_items_according_to_layout(counters, &self.charts.counters_layout, |c| &c.id);
        let counters = proto_v1::Counters {
            counters: counters_sorted,
        };
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
        let resolution_info = chart_entry.resolutions.get(&resolution);
        // settings such as `approximate_trailing_points` and `missing_date_policy`
        // are used in the type underneath the handle
        let query_handle = resolution_info
            .and_then(|enabled_resolution| {
                enabled_resolution.type_specifics.clone().into_line_handle()
            })
            .ok_or_else(|| {
                Status::not_found(format!(
                    "resolution '{}' for line chart '{}' was not found",
                    String::from(resolution),
                    chart_name,
                ))
            })?;

        let from = request
            .from
            .and_then(|date| NaiveDate::from_str(&date).ok())
            .map(|d| day_start(&d));
        let to_exclusive = request
            .to
            .and_then(|date| NaiveDate::from_str(&date).ok())
            .map(|d| day_start(&d.saturating_next_timespan()));
        let request_range = (from..to_exclusive).into();
        let points_limit = Some(self.limits.requested_points_limit);

        let chart_data = self
            .query_line_chart_with_handle(
                chart_name,
                chart_entry,
                query_handle,
                request_range,
                points_limit,
                Utc::now(),
            )
            .await
            .map_err(map_update_error)?;

        Ok(Response::new(chart_data))
    }

    async fn get_line_charts(
        &self,
        _request: Request<proto_v1::GetLineChartsRequest>,
    ) -> Result<Response<proto_v1::LineCharts>, Status> {
        let layout = self.charts.lines_layout.clone();
        let sections = add_chart_info_to_layout(layout, &self.charts.charts_info);

        Ok(Response::new(proto_v1::LineCharts { sections }))
    }
}
