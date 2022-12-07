use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, ChartInt, ChartRequest, Counters, CountersRequest,
    PointInt, Precision,
};
use std::str::FromStr;
use tonic::{Request, Response, Status};

pub struct Service {}

impl Service {
    pub fn new() -> Self {
        Self {}
    }
}

fn generate_intervals(mut start: NaiveDate, precision: Precision) -> Vec<NaiveDate> {
    let now = chrono::offset::Utc::now().naive_utc().date();
    let mut times = vec![];
    while start < now {
        times.push(start);
        match precision {
            Precision::Day => start += Duration::days(1),
            Precision::Month => start = chronoutil::delta::shift_months(start, 1),
        }
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
            total_blocks_all_time: 16075890,
        }))
    }

    async fn get_new_blocks(
        &self,
        request: Request<ChartRequest>,
    ) -> Result<Response<ChartInt>, Status> {
        let request = request.into_inner();
        let start = NaiveDate::from_str("2022-01-01").unwrap();
        let precision = Precision::from_i32(request.precision).unwrap();
        let base_value = match precision {
            Precision::Day => 100,
            Precision::Month => 3000,
        };
        let from = request
            .from
            .map(|date| NaiveDate::parse_from_str(&date, "%d-%m-%Y").unwrap());
        let to = request
            .to
            .map(|date| NaiveDate::parse_from_str(&date, "%d-%m-%Y").unwrap());
        let chart = generate_intervals(start, precision)
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
            .map(|(i, date)| PointInt {
                date: date.format("%d-%m-%Y").to_string(),
                value: base_value + (i as u64 % 100),
            })
            .collect();
        Ok(Response::new(ChartInt { chart }))
    }
}
