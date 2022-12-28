use async_trait::async_trait;
use chrono::NaiveDate;
use sea_orm::{DatabaseConnection, DbErr};
use stats::ReadError;
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, Counters, GetCountersRequest, GetLineChartRequest,
    LineChart,
};
use std::{str::FromStr, sync::Arc};
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct ReadService {
    db: Arc<DatabaseConnection>,
}

impl ReadService {
    pub async fn new(db: Arc<DatabaseConnection>) -> Result<Self, DbErr> {
        Ok(Self { db })
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
        let counters = stats::get_counters(&self.db)
            .await
            .map_err(map_read_error)?;
        Ok(Response::new(counters))
    }

    async fn get_line_chart(
        &self,
        request: Request<GetLineChartRequest>,
    ) -> Result<Response<LineChart>, Status> {
        let request = request.into_inner();
        let from = request
            .from
            .and_then(|date| NaiveDate::from_str(&date).ok());
        let to = request.to.and_then(|date| NaiveDate::from_str(&date).ok());
        let chart = stats::get_chart_data(&self.db, &request.name, from, to)
            .await
            .map_err(map_read_error)?;
        Ok(Response::new(chart))
    }
}
