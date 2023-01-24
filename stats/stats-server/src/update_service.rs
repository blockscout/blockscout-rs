use chrono::Utc;
use cron::Schedule;
use sea_orm::{DatabaseConnection, DbErr};
use std::sync::Arc;

use crate::charts::Charts;

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

    pub async fn update(&self) {
        let _timer = stats::metrics::UPDATE_TIME.start_timer();

        let (full_update, min_block_blockscout) =
            stats::is_blockscout_indexing(&self.blockscout, &self.db)
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("error during blockscout indexing check: {}", e);
                    (true, i64::MAX)
                });
        tracing::info!(full_update = full_update, "start updating all charts");
        let handles = self.charts.charts.iter().map(|chart| {
            let db = self.db.clone();
            let blockscout = self.blockscout.clone();
            let chart = chart.clone();
            tokio::spawn(async move {
                tracing::info!("updating {}", chart.name());
                let result = {
                    let _timer = stats::metrics::CHART_UPDATE_TIME
                        .with_label_values(&[chart.name()])
                        .start_timer();
                    chart.update(&db, &blockscout, full_update).await
                };
                if let Err(err) = result {
                    stats::metrics::UPDATE_ERRORS
                        .with_label_values(&[chart.name()])
                        .inc();
                    tracing::error!("error during updating {}: {}", chart.name(), err);
                } else {
                    tracing::info!("successfully updated chart {}", chart.name());
                }
            })
        });
        futures::future::join_all(handles).await;
        tracing::info!("updating all charts is completed");
        if let Err(e) = stats::set_min_block_saved(&self.db, min_block_blockscout).await {
            tracing::error!("error during saving indexing info: {}", e);
        }
    }

    pub async fn run_cron(self: Arc<Self>, schedule: Schedule) {
        loop {
            let sleep_duration = time_till_next_call(&schedule);
            tracing::debug!("scheduled next run of stats update in {:?}", sleep_duration);
            tokio::time::sleep(sleep_duration).await;
            self.update().await;
        }
    }
}
