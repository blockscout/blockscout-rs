use crate::{charts::Charts, serializers::serialize_line_points};
use async_trait::async_trait;
use chrono::NaiveDate;
use sea_orm::{DatabaseConnection, DbErr};
use stats::ReadError;
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, Counter, Counters, GetCountersRequest, GetLineChartRequest,
    GetLineChartsRequest, LineChart, LineCharts,
};
use std::{str::FromStr, sync::Arc};
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct ReadService {
    db: Arc<DatabaseConnection>,
    charts: Arc<Charts>,
}

impl ReadService {
    pub async fn new(db: Arc<DatabaseConnection>, charts: Arc<Charts>) -> Result<Self, DbErr> {
        Ok(Self { db, charts })
    }
}

fn map_read_error(err: ReadError) -> Status {
    match &err {
        ReadError::NotFound(_) => Status::not_found(err.to_string()),
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
        let mut data = stats::get_counters(&self.db)
            .await
            .map_err(map_read_error)?;

        let counters = self
            .charts
            .config
            .counters
            .iter()
            .filter_map(|counter| {
                self.charts
                    .charts_info
                    .get(&counter.id)
                    .map(|info| (counter, info))
            })
            .filter_map(|(counter, info)| {
                data.remove(&counter.id).map(|point| {
                    let point: stats::DateValue = if info.chart.relevant_or_zero() {
                        point.relevant_or_zero()
                    } else {
                        point
                    };
                    Counter {
                        id: counter.id.clone(),
                        value: point.value,
                        title: counter.title.clone(),
                        description: counter.description.clone(),
                        units: info.settings.units.clone(),
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
            .ok_or_else(|| Status::not_found(format!("chart {} not found", request.name)))?;

        let from = request
            .from
            .and_then(|date| NaiveDate::from_str(&date).ok());
        let to = request.to.and_then(|date| NaiveDate::from_str(&date).ok());
        let policy = Some(chart_info.chart.missing_date_policy());
        let mut data = stats::get_chart_data(&self.db, &request.name, from, to, policy)
            .await
            .map_err(map_read_error)?;

        if chart_info.chart.drop_last_point() {
            // remove last data point, because it can be partially updated
            if let Some(last) = data.last() {
                if last.is_partial() {
                    data.pop();
                }
            }
        }
        let serialized_chart = serialize_line_points(data);
        Ok(Response::new(LineChart {
            chart: serialized_chart,
        }))
    }

    async fn get_line_charts(
        &self,
        _request: Request<GetLineChartsRequest>,
    ) -> Result<Response<LineCharts>, Status> {
        Ok(Response::new(self.charts.config.lines.clone().into()))
    }
}
