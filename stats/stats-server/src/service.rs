use async_trait::async_trait;
use stats_proto::blockscout::stats::v1::{stats_server::Stats, GetStatsRequest, GetStatsResponse};

pub struct StatsService {}

impl StatsService {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Stats for StatsService {
    async fn get_stats(
        &self,
        _request: tonic::Request<GetStatsRequest>,
    ) -> Result<tonic::Response<GetStatsResponse>, tonic::Status> {
        unimplemented!();
    }
}
