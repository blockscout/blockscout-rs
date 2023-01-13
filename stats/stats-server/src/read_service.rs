use async_trait::async_trait;
use chrono::NaiveDate;
use sea_orm::{DatabaseConnection, DbErr};
use serde::Deserialize;
use stats::ReadError;
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, Counter, Counters, GetCountersRequest, GetLineChartRequest,
    GetLineChartsRequest, LineChart, LineCharts, Point,
};
use std::{collections::HashSet, str::FromStr, sync::Arc};
use tonic::{Request, Response, Status};

#[derive(Debug, Clone, Deserialize)]
pub struct CounterInfo {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChartsConfig {
    pub counters: Vec<CounterInfo>,
    pub lines: LineCharts,
}

#[derive(Clone)]
pub struct ReadService {
    db: Arc<DatabaseConnection>,
    charts_config: ChartsConfig,
    charts_filter: HashSet<String>,
}

impl ReadService {
    pub async fn new(
        db: Arc<DatabaseConnection>,
        charts_config: ChartsConfig,
        charts_filter: HashSet<String>,
    ) -> Result<Self, DbErr> {
        Ok(Self {
            db,
            charts_config,
            charts_filter,
        })
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
            .charts_config
            .counters
            .iter()
            .filter_map(|info| {
                data.remove(&info.id).map(|value| Counter {
                    label: info.label.clone(),
                    id: info.id.clone(),
                    value,
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
        if !self.charts_filter.contains(&request.name) {
            return Err(tonic::Status::not_found(format!(
                "chart {} not found",
                request.name
            )));
        }
        let from = request
            .from
            .and_then(|date| NaiveDate::from_str(&date).ok());
        let to = request.to.and_then(|date| NaiveDate::from_str(&date).ok());
        let data = stats::get_chart_data(&self.db, &request.name, from, to)
            .await
            .map_err(map_read_error)?
            .into_iter()
            .map(|point| Point {
                date: point.date.to_string(),
                value: point.value,
            })
            .collect();
        Ok(Response::new(LineChart { chart: data }))
    }

    async fn get_line_charts(
        &self,
        _request: tonic::Request<GetLineChartsRequest>,
    ) -> Result<tonic::Response<LineCharts>, tonic::Status> {
        Ok(Response::new(self.charts_config.lines.clone()))
    }
}
