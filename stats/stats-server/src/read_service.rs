use async_trait::async_trait;
use chrono::NaiveDate;
use sea_orm::{DatabaseConnection, DbErr};
use stats::ReadError;
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, Counter, Counters, GetCountersRequest, GetLineChartRequest,
    GetLineChartsRequest, LineChart, LineCharts, Point,
};
use std::{str::FromStr, sync::Arc};
use tonic::{Request, Response, Status};

use crate::charts::Charts;

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
        ReadError::NotFound(_) => tonic::Status::not_found(err.to_string()),
        _ => tonic::Status::internal(err.to_string()),
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
            .filter_map(|info| {
                data.remove(&info.id).map(|point| {
                    let point = if info.settings.relevant_or_zero {
                        point.relevant_or_zero()
                    } else {
                        point
                    };
                    Counter {
                        id: info.id.clone(),
                        value: point.value,
                        title: info.title.clone(),
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
        if !self.charts.lines_filter.contains(&request.name) {
            return Err(tonic::Status::not_found(format!(
                "chart {} not found",
                request.name
            )));
        }
        let settings =
            self.charts.settings.get(&request.name).ok_or_else(|| {
                tonic::Status::not_found(format!("chart {} not found", request.name))
            })?;

        let from = request
            .from
            .and_then(|date| NaiveDate::from_str(&date).ok());
        let to = request.to.and_then(|date| NaiveDate::from_str(&date).ok());
        let mut data = stats::get_chart_data(&self.db, &request.name, from, to)
            .await
            .map_err(map_read_error)?;

        if settings.drop_last_point {
            // remove last data point, because it can be partially updated
            if let Some(last) = data.last() {
                if last.can_be_partial() {
                    data.pop();
                }
            }
        }

        let serialized_chart: Vec<_> = data
            .into_iter()
            .map(|point| Point {
                date: point.date.to_string(),
                value: point.value,
            })
            .collect();
        Ok(Response::new(LineChart {
            chart: serialized_chart,
        }))
    }

    async fn get_line_charts(
        &self,
        _request: tonic::Request<GetLineChartsRequest>,
    ) -> Result<tonic::Response<LineCharts>, tonic::Status> {
        Ok(Response::new(self.charts.config.lines.clone().into()))
    }
}
