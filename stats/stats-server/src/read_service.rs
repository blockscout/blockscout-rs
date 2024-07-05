use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use crate::{
    config::types,
    runtime_setup::{EnabledChartEntry, RuntimeSetup},
    serializers::serialize_line_points,
    settings::LimitsSettings,
};

use async_trait::async_trait;
use chrono::{Duration, NaiveDate, Utc};
use proto_v1::stats_service_server::StatsService;
use sea_orm::{DatabaseConnection, DbErr};
use stats::{
    entity::sea_orm_active_enums::ChartType, types::ZeroTimespanValue, MissingDatePolicy, ReadError,
};
use stats_proto::blockscout::stats::v1 as proto_v1;
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
    /// See [`LimitsSettings::request_interval_limit_days`]
    pub request_interval_limit: Duration,
}

impl From<LimitsSettings> for ReadLimits {
    fn from(value: LimitsSettings) -> Self {
        Self {
            request_interval_limit: Duration::days(value.request_interval_limit_days.into()),
        }
    }
}

fn map_read_error(err: ReadError) -> Status {
    match &err {
        ReadError::ChartNotFound(_) => Status::not_found(err.to_string()),
        ReadError::IntervalLimitExceeded(_) => Status::invalid_argument(err.to_string()),
        _ => {
            tracing::error!(err = ?err, "internal read error");
            Status::internal(err.to_string())
        }
    }
}

/// Add chart settings to each chart id in layout
///
/// Returns `None` if settings were not found for some chart.
fn add_settings_to_layout(
    layout: Vec<types::LineChartCategory>,
    settings: BTreeMap<String, EnabledChartEntry>,
) -> Vec<proto_v1::LineChartSection> {
    layout
        .into_iter()
        .map(|cat| cat.intersect_settings(&settings))
        .collect()
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
            .filter(|(_, chart)| chart.static_info.chart_type == ChartType::Counter)
            .filter_map(|(name, counter)| {
                data.remove(name).map(|point| {
                    let point: stats::DateValueString =
                        if counter.static_info.missing_date_policy == MissingDatePolicy::FillZero {
                            point.relevant_or_zero(Utc::now().date_naive())
                        } else {
                            point
                        };
                    proto_v1::Counter {
                        id: counter.static_info.name.clone(),
                        value: point.value,
                        title: counter.settings.title.clone(),
                        description: counter.settings.description.clone(),
                        units: counter.settings.units.clone(),
                    }
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
        let chart_info = self
            .charts
            .charts_info
            .get(&request.name)
            .filter(|e| e.static_info.chart_type == ChartType::Line)
            .ok_or_else(|| Status::not_found(format!("chart {} not found", request.name)))?;

        let from = request
            .from
            .and_then(|date| NaiveDate::from_str(&date).ok());
        let to = request.to.and_then(|date| NaiveDate::from_str(&date).ok());
        let policy = chart_info.static_info.missing_date_policy;
        let mark_approx = chart_info.static_info.approximate_trailing_points;
        let interval_limit = Some(self.limits.request_interval_limit);
        let data = stats::get_line_chart_data(
            &self.db,
            &request.name,
            from,
            to,
            interval_limit,
            policy,
            true,
            mark_approx,
        )
        .await
        .map_err(map_read_error)?;

        let serialized_chart = serialize_line_points(data);
        Ok(Response::new(proto_v1::LineChart {
            chart: serialized_chart,
        }))
    }

    async fn get_line_charts(
        &self,
        _request: Request<proto_v1::GetLineChartsRequest>,
    ) -> Result<Response<proto_v1::LineCharts>, Status> {
        let layout = self.charts.lines_layout.clone();
        let settings = self.charts.charts_info.clone();
        let sections = add_settings_to_layout(layout, settings);

        Ok(Response::new(proto_v1::LineCharts { sections }))
    }
}
