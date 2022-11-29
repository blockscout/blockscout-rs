use std::str::FromStr;

use async_trait::async_trait;
use chrono::{Duration, NaiveDateTime};
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, Chart, ChartRequest, Counters, CountersRequest, Point,
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
    async fn get_counters(
        &self,
        _request: Request<CountersRequest>,
    ) -> Result<Response<Counters>, Status> {
        Ok(Response::new(Counters {
            total_blocks_all_time: 16075890,
        }))
    }
    async fn get_new_blocks(
        &self,
        _request: Request<ChartRequest>,
    ) -> Result<Response<Chart>, Status> {
        let start = NaiveDateTime::from_str("2022-01-01T00:00:00").unwrap();
        let chart = (0..90)
            .into_iter()
            .map(|i| Point {
                date: (start + Duration::days(i)).format("%d-%m-%Y").to_string(),
                value: 100 + (i as u64 % 10),
            })
            .collect();
        Ok(Response::new(Chart { chart }))
    }
}
