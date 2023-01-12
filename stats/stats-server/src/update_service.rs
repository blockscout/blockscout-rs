use chrono::Utc;
use cron::Schedule;
use sea_orm::{DatabaseConnection, DbErr};
use stats::Chart;
use std::sync::Arc;

pub struct UpdateService {
    db: Arc<DatabaseConnection>,
    blockscout: Arc<DatabaseConnection>,
    charts: Vec<Arc<dyn Chart + Send + Sync + 'static>>,
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
        charts: Vec<Arc<dyn Chart + Send + Sync + 'static>>,
    ) -> Result<Self, DbErr> {
        Ok(Self {
            db,
            blockscout,
            charts,
        })
    }

    pub async fn update(&self) {
        let full_update = stats::is_blockscout_indexing(&self.blockscout, &self.db)
            .await
            .unwrap_or_else(|e| {
                tracing::error!("error during blockscout indexing check: {}", e);
                true
            });
        tracing::info!(full_update = full_update, "start updating all charts");
        let handles = self.charts.iter().map(|chart| {
            let db = self.db.clone();
            let blockscout = self.blockscout.clone();
            let chart = chart.clone();
            tokio::spawn(async move {
                tracing::info!("updating {}", chart.name());
                let result = chart.update(&db, &blockscout, full_update).await;
                if let Err(err) = result {
                    tracing::error!("error during updating {}: {}", chart.name(), err);
                }
            })
        });
        futures::future::join_all(handles).await;
        if let Err(e) = stats::save_indexing_info(&self.blockscout, &self.db).await {
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
