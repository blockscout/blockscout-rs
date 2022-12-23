use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use sea_orm::{Database, DatabaseConnection, DbErr};
use stats::migration::MigratorTrait;
use stats_proto::blockscout::stats::v1::{
    stats_service_server::StatsService, Counters, GetCountersRequest, GetLineChartRequest,
    LineChart, Point,
};
use std::{collections::HashMap, str::FromStr};
use tonic::{Request, Response, Status};

pub struct Service {
    db: DatabaseConnection,
    #[allow(dead_code)]
    blockscout: DatabaseConnection,
}

impl Service {
    pub async fn new(db_url: &str, blockscout_db_url: &str) -> Result<Self, DbErr> {
        let db = Database::connect(db_url).await?;
        let blockscout = Database::connect(blockscout_db_url).await?;
        Ok(Self { db, blockscout })
    }

    pub async fn migrate(&self) -> Result<(), DbErr> {
        stats::migration::Migrator::up(&self.db, None).await
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
        _request: Request<GetCountersRequest>,
    ) -> Result<Response<Counters>, Status> {
        Ok(Response::new(Counters {
            counters: HashMap::from_iter([
                ("totalBlocksAllTime".into(), "16075890".into()),
                ("averageBlockTime".into(), "12.1".into()),
                ("completedTransactions".into(), "190722170".into()),
                ("totalTransactions".into(), "200122120".into()),
                ("totalAccounts".into(), "123945332".into()),
            ]),
        }))
    }

    async fn get_line_chart(
        &self,
        request: Request<GetLineChartRequest>,
    ) -> Result<Response<LineChart>, Status> {
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
        Ok(Response::new(LineChart { chart }))
    }
}
