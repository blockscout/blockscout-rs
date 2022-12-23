use std::str::FromStr;

use async_trait::async_trait;
use chrono::NaiveDate;
use sea_orm::{Database, DatabaseConnection, DbErr};
use stats::{migration::MigratorTrait, ReadError};
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, Counters, GetCountersRequest, GetLineChartRequest,
    LineChart,
};
use tonic::{Request, Response, Status};

pub struct Service {
    db: DatabaseConnection,
    #[allow(dead_code)]
    blockscout: DatabaseConnection,
}

impl Service {
    pub async fn new(db_url: &str, blockscout_db_url: &str) -> Result<Self, DbErr> {
        let db = Database::connect(db_url).await?;
        let blockscout = Database::connect(blockscout_db_url).await?;
        Ok(Self { db, blockscout })
    }

    pub async fn migrate(&self) -> Result<(), DbErr> {
        stats::migration::Migrator::up(&self.db, None).await
    }
}

fn map_read_error(err: ReadError) -> Status {
    match &err {
        ReadError::NotFound(_) => tonic::Status::not_found(err.to_string()),
        _ => tonic::Status::internal(err.to_string()),
    }
}

#[async_trait]
impl StatsService for Service {
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
        let chart = stats::get_chart_int(&self.db, &request.name, from, to)
            .await
            .map_err(map_read_error)?;
        Ok(Response::new(chart))
    }
}
