use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, Chart, ChartRequest, Counters, CountersRequest, Point,
};
use std::str::FromStr;
use tonic::{Request, Response, Status};

pub struct Service {}

impl Service {
    pub fn new() -> Self {
        Self {}
    }
}

fn generate_intervals(mut start: NaiveDate) -> Vec<NaiveDate> {
    let now = chrono::offset::Utc::now().naive_utc().date();
    let mut times = vec![];
    while start < now {
        times.push(start);
        start += Duration::days(1);
    }
    times
}

#[async_trait]
impl StatsService for Service {
    async fn get_counters(
        &self,
        _request: Request<CountersRequest>,
    ) -> Result<Response<Counters>, Status> {
        Ok(Response::new(Counters {
            total_blocks_all_time: "16075890".into(),
        }))
    }

    async fn get_line(&self, request: Request<ChartRequest>) -> Result<Response<Chart>, Status> {
        let request = request.into_inner();
        let start = NaiveDate::from_str("2022-01-01").unwrap();
        let from = request
            .from
            .map(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d").unwrap());
        let to = request
            .to
            .map(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d").unwrap());
        let chart = generate_intervals(start)
            .into_iter()
            .filter(|date| match from {
                Some(from) => date >= &from,
                None => true,
            })
            .filter(|date| match to {
                Some(to) => date <= &to,
                None => true,
            })
            .enumerate()
            .map(|(i, date)| Point {
                date: date.format("%Y-%m-%d").to_string(),
                // use linear congruental generator as some quick pseudo rand integers
                value: (100 + ((i * 1103515245 + 12345) % 100)).to_string(),
            })
            .collect();
        Ok(Response::new(Chart { chart }))
    }
}
