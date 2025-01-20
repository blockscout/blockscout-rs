use crate::{
    blockscout_waiter::IndexingStatusListener,
    runtime_setup::{RuntimeSetup, UpdateGroupEntry},
};
use chrono::Utc;
use cron::Schedule;
use futures::{stream::FuturesUnordered, StreamExt};
use sea_orm::{DatabaseConnection, DbErr};
use stats::data_source::types::{BlockscoutMigrations, UpdateParameters};
use std::sync::{atomic::AtomicU64, Arc};
use tokio::sync::Semaphore;

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
    /// Run the service in infinite loop.
    /// Terminates dependant threads if enough fail.
    pub async fn run(
        self: Arc<Self>,
        concurrent_initial_tasks: usize,
        default_schedule: Schedule,
        force_update_on_start: Option<bool>,
    ) {
        let initial_update_semaphore: Arc<Semaphore> =
            Arc::new(Semaphore::new(concurrent_initial_tasks));
        let groups = self.charts.update_groups.values();
        let init_update_tracker = InitialUpdateTracker::new(groups.len() as u64);
        let mut group_update_jobs: FuturesUnordered<_> = groups
            .map(|group| {
                let this = self.clone();
                let group_entry = group.clone();
                let default_schedule = default_schedule.clone();
                let status_listener = self.status_listener.clone();
                let initial_update_semaphore = initial_update_semaphore.clone();
                let init_update_tracker = &init_update_tracker;
                async move {
                    Self::wait_for_start_condition(&group_entry, status_listener).await;
                    this.clone()
                        .run_initial_update(
                            &group_entry,
                            force_update_on_start,
                            &initial_update_semaphore,
                            init_update_tracker,
                        )
                        .await;
                    this.run_recurrent_update(group_entry, &default_schedule)
                        .await
                }
            })
            .collect();

        // These futures should never complete because they run in infinite loop.
        // If any completes, it means something went terribly wrong.
        if let Some(()) = group_update_jobs.next().await {
            tracing::error!("update job stopped unexpectedly");
            panic!("update job stopped unexpectedly");
        }
    }

    async fn wait_for_start_condition(
        group_entry: &UpdateGroupEntry,
        status_listener: Option<IndexingStatusListener>,
    ) {
        if let Some(mut status_listener) = status_listener {
            let wait_result = status_listener
                .wait_until_status_at_least(
                    group_entry
                        .group
                        .dependency_indexing_status_requirement(&group_entry.enabled_members),
                )
                .await;
            if wait_result.is_err() {
                panic!(
                    "Indexing status listener channel closed unexpectedly. \
                    This indicates that the status aggregator has stopped running."
                );
            }
        }
    }

    async fn run_initial_update(
        self: Arc<Self>,
        group_entry: &UpdateGroupEntry,
        force_update_on_start: Option<bool>,
        initial_update_semaphore: &Semaphore,
        init_update_tracker: &InitialUpdateTracker,
    ) {
        {
            let _init_update_permit = initial_update_semaphore
                .acquire()
                .await
                .expect("failed to acquire permit");
            if let Some(force_full) = force_update_on_start {
                self.update(group_entry.clone(), force_full).await
            };
        }
        tracing::info!(
            update_group = group_entry.group.name(),
            "initial update for group is done"
        );
        init_update_tracker.mark_updated();
        init_update_tracker.report();
    }

    async fn run_recurrent_update(
        self: &Arc<Self>,
        group_entry: UpdateGroupEntry,
        default_schedule: &Schedule,
    ) {
        let this = self.clone();
        let chart = group_entry.clone();
        let schedule = group_entry
            .update_schedule
            .as_ref()
            .unwrap_or(default_schedule)
            .clone();
        this.run_cron(chart, schedule).await
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

/// Reports progress of inital updates to logs
struct InitialUpdateTracker {
    updated_groups: AtomicU64,
    total_groups: u64,
}

impl InitialUpdateTracker {
    pub fn new(total_groups: u64) -> Self {
        Self {
            updated_groups: AtomicU64::new(0),
            total_groups,
        }
    }

    pub fn mark_updated(&self) {
        self.updated_groups
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn report(&self) {
        tracing::info!(
            "{}/{} of initial updates are finished",
            self.updated_groups
                .load(std::sync::atomic::Ordering::Relaxed),
            self.total_groups
        );
    }
}
