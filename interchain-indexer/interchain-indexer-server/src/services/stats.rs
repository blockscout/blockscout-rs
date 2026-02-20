use crate::proto::{interchain_statistics_service_server::*, *};
use chrono::{DateTime, Utc};
use interchain_indexer_logic::InterchainDatabase;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct InterchainStatisticsServiceImpl {
    pub db: Arc<InterchainDatabase>,
}

impl InterchainStatisticsServiceImpl {
    pub fn new(db: Arc<InterchainDatabase>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl InterchainStatisticsService for InterchainStatisticsServiceImpl {
    async fn get_common_statistics(
        &self,
        request: Request<GetCommonStatisticsRequest>,
    ) -> Result<Response<GetCommonStatisticsResponse>, Status> {
        let inner = request.into_inner();
        let timestamp = inner
            .timestamp
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0).map(|dt| dt.naive_utc()))
            .unwrap_or_else(|| Utc::now().naive_utc());

        let counters = self
            .db
            .get_total_counters(timestamp, None, None)
            .await
            .map_err(map_stats_error)?;

        let response = GetCommonStatisticsResponse {
            timestamp: super::utils::db_datetime_to_string(counters.timestamp),
            total_messages: counters.total_messages,
            total_transfers: counters.total_transfers,
        };
        Ok(Response::new(response))
    }

    async fn get_daily_statistics(
        &self,
        request: Request<GetDailyStatisticsRequest>,
    ) -> Result<Response<GetDailyStatisticsResponse>, Status> {
        let inner = request.into_inner();
        let timestamp = inner
            .timestamp
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0).map(|dt| dt.naive_utc()))
            .unwrap_or_else(|| Utc::now().naive_utc());

        let counters = self
            .db
            .get_daily_counters(timestamp, None, None)
            .await
            .map_err(map_stats_error)?;

        let response = GetDailyStatisticsResponse {
            date: counters.date.to_string(),
            daily_messages: counters.daily_messages,
            daily_transfers: counters.daily_transfers,
        };
        Ok(Response::new(response))
    }
}

fn map_stats_error(err: anyhow::Error) -> Status {
    Status::internal(err.to_string())
}
