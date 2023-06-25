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
    pub async fn force_async_update_and_run(
        self: Arc<Self>,
        concurrent_tasks: usize,
        default_schedule: Schedule,
        force_update_on_start: Option<bool>,
    ) {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrent_tasks));
        let tasks = self
            .charts
            .charts_info
            .values()
            .map(|chart_info| {
                let this = self.clone();
                let chart = chart_info.chart.clone();
                let default_schedule = default_schedule.clone();
                let sema = semaphore.clone();
                async move {
                    let _permit = sema.acquire().await.expect("failed to acquire permit");
                    if let Some(force_full) = force_update_on_start {
                        this.clone().update(chart.clone(), force_full).await
                    };
                    this.spawn_chart_updater(chart, &default_schedule);
                }
            })
            .collect::<Vec<_>>();
        futures::future::join_all(tasks).await;
        tracing::info!("initial updating is done");
    }

    fn spawn_chart_updater(self: &Arc<Self>, chart: ArcChart, default_schedule: &Schedule) {
        let chart_info = self
            .charts
            .charts_info
            .get(chart.name())
            .expect("enabled chart must contain settings");
        let this = self.clone();
        let chart = chart.clone();
        let schedule = chart_info
            .settings
            .update_schedule
            .as_ref()
            .unwrap_or(default_schedule)
            .clone();
        tokio::spawn(async move { this.run_cron(chart, schedule).await });
    }

    async fn update(self: Arc<Self>, chart: ArcChart, force_full: bool) {
        tracing::info!(chart = chart.name(), "updating chart");
        let result = {
            let _timer = stats::metrics::CHART_UPDATE_TIME
                .with_label_values(&[chart.name()])
                .start_timer();
            chart
                .update_with_mutex(&self.db, &self.blockscout, force_full)
                .await
        };
        if let Err(err) = result {
            stats::metrics::UPDATE_ERRORS
                .with_label_values(&[chart.name()])
                .inc();
            tracing::error!(chart = chart.name(), "error during updating chart: {}", err);
        } else {
            tracing::info!(chart = chart.name(), "successfully updated chart");
        }
    }

    async fn run_cron(self: Arc<Self>, chart: ArcChart, schedule: Schedule) {
        loop {
            let sleep_duration = time_till_next_call(&schedule);
            tracing::info!(
                chart = chart.name(),
                "scheduled next run of chart update in {:?}",
                sleep_duration
            );
            tokio::time::sleep(sleep_duration).await;
            self.clone().update(chart.clone(), false).await;
        }
    }
}
