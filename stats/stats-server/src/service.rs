use std::str::FromStr;

use async_trait::async_trait;
use chrono::{Datelike, Duration, NaiveDateTime};
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, ChartInt, ChartRequest, Counters, CountersRequest,
    PointInt, Precision,
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
        request: Request<ChartRequest>,
    ) -> Result<Response<ChartInt>, Status> {
        let request = request.into_inner();
        let start = NaiveDateTime::from_str("2022-01-01T00:00:00").unwrap();
        let chart = match Precision::from_i32(request.precision).unwrap() {
            Precision::Day => (0..90)
                .into_iter()
                .map(|i| PointInt {
                    date: (start + Duration::days(i)).format("%d-%m-%Y").to_string(),
                    value: 100 + (i as u64 % 10),
                })
                .collect(),
            Precision::Month => (0..3)
                .into_iter()
                .map(|i| PointInt {
                    date: start
                        .with_month(start.month() + i)
                        .unwrap()
                        .format("%d-%m-%Y")
                        .to_string(),
                    value: 3000 + (i as u64 % 10),
                })
                .collect(),
        };
        Ok(Response::new(ChartInt { chart }))
    }
}
