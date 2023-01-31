use crate::charts::{ArcChart, Charts};
use chrono::Utc;
use cron::Schedule;
use sea_orm::{DatabaseConnection, DbErr};
use std::sync::Arc;

pub struct UpdateService {
    db: Arc<DatabaseConnection>,
    blockscout: Arc<DatabaseConnection>,
    charts: Arc<Charts>,
}

fn time_till_next_call(schedule: &Schedule) -> std::time::Duration {
    let default = std::time::Duration::from_millis(500);
    let now = Utc::now();

    schedule
        .upcoming(Utc)
        .next()
        .map_or(default, |t| (t - now).to_std().unwrap_or(default))
}

impl UpdateService {
    pub async fn new(
        db: Arc<DatabaseConnection>,
        blockscout: Arc<DatabaseConnection>,
        charts: Arc<Charts>,
    ) -> Result<Self, DbErr> {
        Ok(Self {
            db,
            blockscout,
            charts,
        })
    }

    pub async fn force_update_all(self: Arc<Self>, force_full: bool) {
        let tasks = self.charts.charts.iter().map(|chart| {
            let this = self.clone();
            let chart = chart.clone();
            tokio::spawn(async move { this.update(chart, force_full).await })
        });
        futures::future::join_all(tasks).await;
    }

    pub fn run(self: Arc<Self>, default_schedule: Schedule) {
        for chart in self.charts.charts.iter() {
            let settings = self
                .charts
                .settings
                .get(chart.name())
                .expect("enabled chart must contain settings");
            {
                let this = self.clone();
                let chart = chart.clone();
                let schedule = settings
                    .update_schedule
                    .as_ref()
                    .unwrap_or(&default_schedule)
                    .clone();
                tokio::spawn(async move { this.run_cron(chart, schedule).await });
            }
        }
    }

    async fn update(&self, chart: ArcChart, force_full: bool) {
        tracing::info!("updating {}", chart.name());
        let result = {
            let _timer = stats::metrics::CHART_UPDATE_TIME
                .with_label_values(&[chart.name()])
                .start_timer();
            chart.update(&self.db, &self.blockscout, force_full).await
        };
        if let Err(err) = result {
            stats::metrics::UPDATE_ERRORS
                .with_label_values(&[chart.name()])
                .inc();
            tracing::error!("error during updating {}: {}", chart.name(), err);
        } else {
            tracing::info!("successfully updated chart {}", chart.name());
        }
    }

    async fn run_cron(&self, chart: ArcChart, schedule: Schedule) {
        loop {
            let sleep_duration = time_till_next_call(&schedule);
            tracing::info!(
                "scheduled next run of chart {} update in {:?}",
                chart.name(),
                sleep_duration
            );
            tokio::time::sleep(sleep_duration).await;
            self.update(chart.clone(), false).await;
        }
    }
}
