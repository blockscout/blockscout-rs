use async_trait::async_trait;
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, Chart, ChartRequest, Point, Stats, StatsRequest,
};
use tonic::{Request, Response, Status};

pub struct Service {}

impl Service {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl StatsService for Service {
    async fn get_stats(&self, _request: Request<StatsRequest>) -> Result<Response<Stats>, Status> {
        Ok(Response::new(Stats {
            total_blocks_all_time: 1,
        }))
    }
    async fn get_new_blocks(
        &self,
        _request: Request<ChartRequest>,
    ) -> Result<Response<Chart>, Status> {
        Ok(Response::new(Chart {
            chart: vec![Point {
                date: "01-01-2022".into(),
                value: 123,
            }],
        }))
    }
}
