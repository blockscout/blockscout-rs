use crate::proto::{interchain_statistics_service_server::*, *};
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
        _request: Request<GetCommonStatisticsRequest>,
    ) -> Result<Response<GetCommonStatisticsResponse>, Status> {
        let response = GetCommonStatisticsResponse {
            timestamp: 0,
            total_messages: 0,
            total_transfers: 0,
        };
        Ok(Response::new(response))
    }

    async fn get_daily_statistics(
        &self,
        _request: Request<GetDailyStatisticsRequest>,
    ) -> Result<Response<GetDailyStatisticsResponse>, Status> {
        let response = GetDailyStatisticsResponse {
            timestamp: 0,
            daily_messages: 0,
            daily_transfers: 0,
        };
        Ok(Response::new(response))
    }
}
