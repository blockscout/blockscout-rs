use crate::charts::Charts;
use chrono::Utc;
use cron::Schedule;
use sea_orm::{DatabaseConnection, DbErr};
use stats::data_source::{group::SyncUpdateGroup, types::UpdateParameters};
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
            .update_groups
            .values()
            .map(|group| {
                let this = self.clone();
                let update_group = group.clone();
                let default_schedule = default_schedule.clone();
                let sema = semaphore.clone();
                async move {
                    let _permit = sema.acquire().await.expect("failed to acquire permit");
                    if let Some(force_full) = force_update_on_start {
                        this.clone().update(update_group.clone(), force_full).await
                    };
                    this.spawn_group_updater(update_group, &default_schedule);
                }
            })
            .collect::<Vec<_>>();
        futures::future::join_all(tasks).await;
        tracing::info!("initial update is done");
    }

    fn spawn_group_updater(
        self: &Arc<Self>,
        update_group: SyncUpdateGroup,
        default_schedule: &Schedule,
    ) {
        let group_info = self
            .charts
            .update_schedule_config
            .update_groups
            .get(&update_group.inner.name())
            .expect("enabled chart must contain settings");
        let this = self.clone();
        let chart = update_group.clone();
        let schedule = group_info
            .update_schedule
            .as_ref()
            .unwrap_or(default_schedule)
            .clone();
        tokio::spawn(async move { this.run_cron(chart, schedule).await });
    }

    async fn update(self: Arc<Self>, update_group: SyncUpdateGroup, force_full: bool) {
        tracing::info!(
            update_group = update_group.inner.name(),
            "updating group of charts"
        );
        let result = {
            let update_parameters = UpdateParameters {
                db: &self.db,
                blockscout: &self.blockscout,
                current_time: chrono::Utc::now(),
                force_full,
            };
            update_group
                .update_charts_with_mutexes(update_parameters, &self.charts.enabled_set)
                .await
        };
        if let Err(err) = result {
            tracing::error!(
                update_group = update_group.inner.name(),
                "error during updating group: {}",
                err
            );
        } else {
            tracing::info!(
                update_group = update_group.inner.name(),
                "successfully updated group"
            );
        }
    }

    async fn run_cron(self: Arc<Self>, update_group: SyncUpdateGroup, schedule: Schedule) {
        loop {
            let sleep_duration = time_till_next_call(&schedule);
            tracing::info!(
                update_group = update_group.inner.name(),
                "scheduled next run of group update in {:?}",
                sleep_duration
            );
            tokio::time::sleep(sleep_duration).await;
            self.clone().update(update_group.clone(), false).await;
        }
    }
}
