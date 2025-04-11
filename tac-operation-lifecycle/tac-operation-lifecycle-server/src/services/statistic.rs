use std::sync::Arc;

use crate::proto::{
    tac_statistic_server::TacStatistic, GetIndexerStatisticRequest, GetIndexerStatisticResponse,
};

use tac_operation_lifecycle_logic::database::TacDatabase;

pub struct StatisticService {
    db: Arc<TacDatabase>,
    realtime_boundary: u64,
}

impl StatisticService {
    pub fn new(db: Arc<TacDatabase>, realtime_boundary: u64) -> Self {
        Self {
            db,
            realtime_boundary,
        }
    }
}

#[async_trait::async_trait]
impl TacStatistic for StatisticService {
    async fn get_indexer_statistics(
        &self,
        _: tonic::Request<GetIndexerStatisticRequest>,
    ) -> Result<tonic::Response<GetIndexerStatisticResponse>, tonic::Status> {
        let now = chrono::Utc::now().timestamp() as u64;
        match self.db.get_statistic(self.realtime_boundary).await {
            Ok(stat) => Ok(tonic::Response::new(GetIndexerStatisticResponse {
                timestamp: now,
                watermark: stat.watermark,
                realtime_from: self.realtime_boundary,
                first_timestamp: stat.first_timestamp,
                last_timestamp: stat.last_timestamp,
                total_intervals: stat.total_intervals as u64,
                failed_intervals: stat.failed_intervals as u64,
                total_pending_intervals: stat.total_pending_intervals as u64,
                historical_pending_intervals: stat.historical_pending_intervals as u64,
                realtime_pending_intervals: stat.realtime_pending_intervals as u64,
                historical_processed_period: stat.historical_processed_period,
                realtime_processed_period: stat.realtime_processed_period,
                historical_sync_completeness: stat.historical_processed_period as f64
                    / (self.realtime_boundary - stat.first_timestamp) as f64,
                realtime_sync_completeness: stat.realtime_processed_period as f64
                    / (now - self.realtime_boundary) as f64,
                total_operations: stat.total_operations as u64,
                failed_operations: stat.failed_operations as u64,
                total_pending_operations: stat.total_pending_operations as u64,
                operations_sync_completeness: (stat.total_operations
                    - stat.failed_operations
                    - stat.total_pending_operations)
                    as f64
                    / stat.total_operations as f64,
            })),

            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }
}
