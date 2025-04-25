use std::sync::Arc;

use tac_operation_lifecycle_logic::database::{
    IntervalDbStatistic, OperationDbStatistic, TacDatabase,
};
use tac_operation_lifecycle_proto::blockscout::tac_operation_lifecycle::v1::{
    tac_statistic_server::TacStatistic, GetFullStatisticRequest, GetFullStatisticResponse,
    GetIntervalStatisticsRequest, GetIntervalStatisticsResponse, GetOperationStatisticsRequest,
    GetOperationStatisticsResponse, IntervalStatistic, OperationStatistic,
};

pub struct StatisticService {
    db: Arc<TacDatabase>,
}

impl StatisticService {
    pub fn new(db: Arc<TacDatabase>) -> Self {
        Self { db }
    }
}

pub fn prepare_intervals_resp(stat: IntervalDbStatistic, now: u64) -> IntervalStatistic {
    IntervalStatistic {
        first_timestamp: stat.first_timestamp,
        last_timestamp: stat.last_timestamp,
        total_intervals: stat.total_intervals as u64,
        pending_intervals: stat.pending_intervals as u64,
        processing_intervals: stat.processing_intervals as u64,
        finalized_intervals: stat.finalized_intervals as u64,
        failed_intervals: stat.failed_intervals as u64,
        finalized_period: stat.finalized_period,
        sync_completeness: stat.finalized_period as f64 / (now - stat.first_timestamp) as f64,
    }
}

pub fn prepare_operations_resp(stat: OperationDbStatistic) -> OperationStatistic {
    OperationStatistic {
        last_timestamp: stat.max_timestamp,
        total_operations: stat.total_operations as u64,
        pending_operations: stat.pending_operations as u64,
        processing_operations: stat.processing_operations as u64,
        finalized_operations: stat.finalized_operations as u64,
        failed_operations: stat.failed_operations as u64,
        sync_completeness: stat.finalized_operations as f64 / stat.total_operations as f64,
    }
}

#[async_trait::async_trait]
impl TacStatistic for StatisticService {
    async fn get_interval_statistics(
        &self,
        _: tonic::Request<GetIntervalStatisticsRequest>,
    ) -> Result<tonic::Response<GetIntervalStatisticsResponse>, tonic::Status> {
        let now = chrono::Utc::now().timestamp() as u64;
        match self.db.get_intervals_statistic().await {
            Ok(stat) => Ok(tonic::Response::new(GetIntervalStatisticsResponse {
                timestamp: now,
                intervals: Some(prepare_intervals_resp(stat, now)),
            })),

            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }

    async fn get_operation_statistics(
        &self,
        _: tonic::Request<GetOperationStatisticsRequest>,
    ) -> Result<tonic::Response<GetOperationStatisticsResponse>, tonic::Status> {
        let now = chrono::Utc::now().timestamp() as u64;
        match self.db.get_operations_statistic().await {
            Ok(stat) => Ok(tonic::Response::new(GetOperationStatisticsResponse {
                timestamp: now,
                operations: Some(prepare_operations_resp(stat)),
            })),

            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }

    async fn get_full_statistics(
        &self,
        _: tonic::Request<GetFullStatisticRequest>,
    ) -> Result<tonic::Response<GetFullStatisticResponse>, tonic::Status> {
        let now = chrono::Utc::now().timestamp() as u64;
        let op_stat = self.db.get_operations_statistic();
        match self.db.get_intervals_statistic().await {
            Ok(int_stat) => Ok(tonic::Response::new(GetFullStatisticResponse {
                timestamp: now,
                watermark: self.db.get_watermark().await.unwrap(),
                intervals: Some(prepare_intervals_resp(int_stat, now)),
                operations: Some(prepare_operations_resp(op_stat.await.unwrap())),
            })),

            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }
}
