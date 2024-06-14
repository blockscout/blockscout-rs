use std::{str::FromStr, sync::Arc};

use crate::{
    runtime_setup::RuntimeSetup, serializers::serialize_line_points, settings::LimitsSettings,
};

use async_trait::async_trait;
use chrono::{Duration, NaiveDate, Utc};
use sea_orm::{DatabaseConnection, DbErr};
use stats::{entity::sea_orm_active_enums::ChartType, MissingDatePolicy, ReadError, ZeroDateValue};
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, Counter, Counters, GetCountersRequest, GetLineChartRequest,
    GetLineChartsRequest, LineChart, LineCharts,
};
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

#[async_trait]
impl StatsService for ReadService {
    async fn get_counters(
        &self,
        _request: Request<GetCountersRequest>,
    ) -> Result<Response<Counters>, Status> {
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
                    Counter {
                        id: counter.static_info.name.clone(),
                        value: point.value,
                        title: counter.settings.title.clone(),
                        description: counter.settings.description.clone(),
                        units: counter.settings.units.clone(),
                    }
                })
            })
            .collect();
        let counters = Counters { counters };
        Ok(Response::new(counters))
    }

    async fn get_line_chart(
        &self,
        request: Request<GetLineChartRequest>,
    ) -> Result<Response<LineChart>, Status> {
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
        Ok(Response::new(LineChart {
            chart: serialized_chart,
        }))
    }

    async fn get_line_charts(
        &self,
        _request: Request<GetLineChartsRequest>,
    ) -> Result<Response<LineCharts>, Status> {
        Ok(Response::new(self.charts.lines_layout.clone().into()))
    }
}
