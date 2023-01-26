use chrono::Utc;
use cron::Schedule;
use sea_orm::{DatabaseConnection, DbErr};
use std::sync::Arc;

use crate::charts::{ArcChart, Charts};

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

    async fn update(&self, chart: ArcChart) {
        // TODO: store min_block_blockscout for each chart
        let (full_update, min_block_blockscout) =
            stats::is_blockscout_indexing(&self.blockscout, &self.db)
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("error during blockscout indexing check: {}", e);
                    (true, i64::MAX)
                });
        tracing::info!(full_update = full_update, "updating {}", chart.name());
        let result = {
            let _timer = stats::metrics::CHART_UPDATE_TIME
                .with_label_values(&[chart.name()])
                .start_timer();
            chart.update(&self.db, &self.blockscout, full_update).await
        };
        if let Err(err) = result {
            stats::metrics::UPDATE_ERRORS
                .with_label_values(&[chart.name()])
                .inc();
            tracing::error!("error during updating {}: {}", chart.name(), err);
        } else {
            tracing::info!("successfully updated chart {}", chart.name());
        }
        // TODO: store min_block_blockscout for each chart
        if let Err(e) = stats::set_min_block_saved(&self.db, min_block_blockscout).await {
            tracing::error!("error during saving indexing info: {}", e);
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
            self.update(chart.clone()).await;
        }
    }

    pub fn force_update_all(self: Arc<Self>) {
        for chart in self.charts.charts.iter() {
            {
                let this = self.clone();
                let chart = chart.clone();
                tokio::spawn(async move { this.update(chart).await });
            }
        }
    }

    pub fn run(self: Arc<Self>, default_schedule: Schedule) {
        for chart in self.charts.charts.iter() {
            let settings = self.charts.settings.get(chart.name()).unwrap();
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
}
