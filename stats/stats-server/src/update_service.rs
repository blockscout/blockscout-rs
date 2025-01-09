use crate::{
    blockscout_waiter::IndexingStatusListener,
    runtime_setup::{RuntimeSetup, UpdateGroupEntry},
};
use chrono::Utc;
use cron::Schedule;
use sea_orm::{DatabaseConnection, DbErr};
use stats::data_source::types::{BlockscoutMigrations, UpdateParameters};
use std::sync::Arc;
use tokio::task::JoinHandle;

const FAILED_UPDATERS_UNTIL_PANIC: u64 = 3;

pub struct UpdateService {
    db: Arc<DatabaseConnection>,
    blockscout_db: Arc<DatabaseConnection>,
    charts: Arc<RuntimeSetup>,
    status_listener: Option<IndexingStatusListener>,
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
        blockscout_db: Arc<DatabaseConnection>,
        charts: Arc<RuntimeSetup>,
        status_listener: Option<IndexingStatusListener>,
    ) -> Result<Self, DbErr> {
        Ok(Self {
            db,
            blockscout_db,
            charts,
            status_listener,
        })
    }

    /// The main function of the service.
    ///
    /// Perform initial update and run the service in infinite loop.
    /// Terminates dependant threads if one fails.
    pub async fn run(
        self: Arc<Self>,
        concurrent_tasks: usize,
        default_schedule: Schedule,
        force_update_on_start: Option<bool>,
    ) {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrent_tasks));
        let (tasks, mut updaters) = self
            .charts
            .update_groups
            .values()
            .map(|group| {
                let this = self.clone();
                let group_entry = group.clone();
                let default_schedule = default_schedule.clone();
                let status_listener = self.status_listener.clone();
                let sema = semaphore.clone();
                (
                    async move {
                        if let Some(mut status_listener) = status_listener {
                            let wait_result = status_listener
                                .wait_until_status_at_least(
                                    group_entry.indexing_status_requirement(),
                                )
                                .await;
                            if wait_result.is_err() {
                                panic!("Indexing status listener channel closed");
                            }
                        }

                        let _permit = sema.acquire().await.expect("failed to acquire permit");
                        if let Some(force_full) = force_update_on_start {
                            this.clone().update(group_entry.clone(), force_full).await
                        };
                    },
                    // todo: wait until initial update is finished
                    self.spawn_group_updater(group.clone(), &default_schedule),
                )
            })
            .collect::<(Vec<_>, Vec<_>)>();
        futures::future::join_all(tasks).await;
        tracing::info!("initial update is done");

        let mut failed = 0;
        while !updaters.is_empty() {
            let (res, _, others) = futures::future::select_all(updaters).await;
            updaters = others;
            tracing::error!("updater stopped: {:?}", res);

            failed += 1;
            if failed >= FAILED_UPDATERS_UNTIL_PANIC {
                panic!("too many critically failed updaters");
            }
        }
    }

    fn spawn_group_updater(
        self: &Arc<Self>,
        group_entry: UpdateGroupEntry,
        default_schedule: &Schedule,
    ) -> JoinHandle<()> {
        let this = self.clone();
        let chart = group_entry.clone();
        let schedule = group_entry
            .update_schedule
            .as_ref()
            .unwrap_or(default_schedule)
            .clone();
        tokio::spawn(this.run_cron(chart, schedule))
    }

    async fn update(self: Arc<Self>, group_entry: UpdateGroupEntry, force_full: bool) {
        tracing::info!(
            // instrumentation is inside `update_charts_with_mutexes`
            update_group = group_entry.group.name(),
            force_update = force_full,
            "updating group of charts"
        );
        let Ok(active_migrations) = BlockscoutMigrations::query_from_db(&self.blockscout_db)
            .await
            .inspect_err(|err| {
                tracing::error!("error during blockscout migrations detection: {:?}", err)
            })
        else {
            return;
        };
        let update_parameters = UpdateParameters {
            db: &self.db,
            blockscout: &self.blockscout_db,
            blockscout_applied_migrations: active_migrations,
            update_time_override: None,
            force_full,
        };
        let result = group_entry
            .group
            .update_charts_with_mutexes(update_parameters, &group_entry.enabled_members)
            .await;
        if let Err(err) = result {
            tracing::error!(
                update_group = group_entry.group.name(),
                "error during updating group: {}",
                err
            );
        } else {
            tracing::info!(
                update_group = group_entry.group.name(),
                "successfully updated group"
            );
        }
    }

    async fn run_cron(self: Arc<Self>, group_entry: UpdateGroupEntry, schedule: Schedule) {
        loop {
            let sleep_duration = time_till_next_call(&schedule);
            tracing::info!(
                update_group = group_entry.group.name(),
                "scheduled next run of group update in {:?}",
                sleep_duration
            );
            tokio::time::sleep(sleep_duration).await;
            self.clone().update(group_entry.clone(), false).await;
        }
    }
}
