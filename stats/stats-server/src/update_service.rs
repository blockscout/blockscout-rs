use chrono::{NaiveDate, Utc};
use cron::Schedule;
use futures::{stream::FuturesUnordered, StreamExt};
use itertools::Itertools;
use sea_orm::{DatabaseConnection, DbErr};
use stats_proto::blockscout::stats::v1 as proto_v1;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex, Semaphore};

use crate::{
    blockscout_waiter::IndexingStatusListener,
    runtime_setup::{RuntimeSetup, UpdateGroupEntry},
    InitialUpdateTracker,
};
use stats::{
    data_source::types::{BlockscoutMigrations, UpdateParameters},
    ChartKey,
};

use std::{collections::HashSet, sync::Arc};

pub struct UpdateService {
    db: Arc<DatabaseConnection>,
    blockscout_db: Arc<DatabaseConnection>,
    charts: Arc<RuntimeSetup>,
    status_listener: Option<IndexingStatusListener>,
    init_update_tracker: InitialUpdateTracker,
    // currently only accessed in one place, but `Mutex`es
    // are needed due to `Arc<Self>` everywhere to provide
    // interior mutability
    on_demand_sender: Mutex<mpsc::Sender<OnDemandReupdateRequest>>,
    on_demand_receiver: Mutex<mpsc::Receiver<OnDemandReupdateRequest>>,
}

fn time_till_next_call(schedule: &Schedule) -> std::time::Duration {
    let default = std::time::Duration::from_millis(500);
    let now = Utc::now();

    schedule
        .upcoming(Utc)
        .next()
        .map_or(default, |t| (t - now).to_std().unwrap_or(default))
}

fn group_update_schedule<'a>(
    group: &'a UpdateGroupEntry,
    default_schedule: &'a Schedule,
) -> &'a Schedule {
    group.update_schedule.as_ref().unwrap_or(default_schedule)
}

impl UpdateService {
    pub async fn new(
        db: Arc<DatabaseConnection>,
        blockscout_db: Arc<DatabaseConnection>,
        charts: Arc<RuntimeSetup>,
        status_listener: Option<IndexingStatusListener>,
    ) -> Result<Self, DbErr> {
        let on_demand = mpsc::channel(128);
        let init_update_tracker = Self::initialize_update_tracker(&charts);
        Ok(Self {
            db,
            blockscout_db,
            charts,
            status_listener,
            init_update_tracker,
            on_demand_sender: Mutex::new(on_demand.0),
            on_demand_receiver: Mutex::new(on_demand.1),
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
        let mut group_update_jobs: FuturesUnordered<_> = groups
            .map(|group| {
                let this = self.clone();
                let group_entry = group.clone();
                let schedule = group_update_schedule(&group_entry, &default_schedule).clone();
                let status_listener = self.status_listener.clone();
                let initial_update_semaphore = initial_update_semaphore.clone();
                let init_update_tracker = &self.init_update_tracker;
                async move {
                    // also includes wait for mutex in `run_initial_update`
                    init_update_tracker
                        .mark_waiting_for_starting_condition(&group_entry.enabled_members)
                        .await;
                    Self::wait_for_start_condition(&group_entry, status_listener).await;
                    this.clone()
                        .run_initial_update(
                            &group_entry,
                            force_update_on_start,
                            &initial_update_semaphore,
                            init_update_tracker,
                        )
                        .await;
                    this.run_recurrent_update(group_entry, schedule).await
                }
            })
            .collect();
        let on_demand_job = self.run_on_demand_executor(&default_schedule);

        // The futures should never complete because they run in infinite loop.
        // If any completes, it means something went terribly wrong.
        let msg = tokio::select! {
        _ = group_update_jobs.next() => {
            "update job stopped unexpectedly"
        }
        _ = on_demand_job => {
            "on demand updater stopped unexpectedly"
        }};
        tracing::error!(msg);
        panic!("{}", msg);
    }

    fn initialize_update_tracker(charts: &RuntimeSetup) -> InitialUpdateTracker {
        let all_charts_requirements = charts.all_enabled_members_indexing_status_requirements();
        InitialUpdateTracker::new(&all_charts_requirements)
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
            init_update_tracker
                .mark_queued_for_initial_update(&group_entry.enabled_members)
                .await;
            init_update_tracker.report().await;
            let _init_update_permit = initial_update_semaphore
                .acquire()
                .await
                .expect("failed to acquire permit");
            init_update_tracker
                .mark_started_initial_update(&group_entry.enabled_members)
                .await;
            init_update_tracker.report().await;
            if let Some(force_full) = force_update_on_start {
                self.clone()
                    .update(group_entry.clone(), force_full, None)
                    .await
            };
        }
        tracing::info!(
            update_group = group_entry.group.name(),
            "initial update for group is done"
        );
        init_update_tracker
            .mark_initial_update_done(&group_entry.enabled_members)
            .await;
        init_update_tracker.report().await;
    }

    async fn run_on_demand_executor(self: &Arc<Self>, default_schedule: &Schedule) {
        let enabled_keys: HashSet<ChartKey> = self
            .charts
            .update_groups
            .values()
            .flat_map(|g| g.enabled_members.iter())
            .cloned()
            .collect();
        loop {
            let Some(reupdate) = self.on_demand_receiver.lock().await.recv().await else {
                tracing::error!("no more on demand reupdate channel senders");
                return;
            };
            tracing::info!(
                request =? reupdate,
                "received an on-demand request for chart reupdate"
            );
            let mut enabled_charts_to_update: HashSet<_> = reupdate
                .chart_names
                .into_iter()
                .filter(|c| enabled_keys.contains(c))
                .collect();

            tracing::info!(
                "{} charts to handle reupdate for",
                enabled_charts_to_update.len()
            );
            while !enabled_charts_to_update.is_empty() {
                let updated = self
                    .reupdate_the_best_matching_group(
                        &enabled_charts_to_update,
                        reupdate.from,
                        reupdate.update_later,
                        default_schedule,
                    )
                    .await;
                if updated.is_empty() {
                    tracing::warn!("on-demand update list was incorrectly filtered and prepared. this is likely a bug");
                    break;
                }
                let mut any_removed = false;
                for u in updated {
                    enabled_charts_to_update.remove(&u);
                    any_removed = true;
                }
                if !any_removed {
                    // should always have something to remove but placed it just in case
                    // to prevent infinite loop
                    tracing::warn!("on-demand updated list does not intersect with enabled charts list. this is likely a bug");
                }

                tracing::info!(
                    charts_to_update_left = enabled_charts_to_update.len(),
                    "finished a step of on-demand update"
                );
            }
            tracing::info!("finished on-demand update");
        }
    }

    /// Returns updated charts
    async fn reupdate_the_best_matching_group(
        self: &Arc<Self>,
        enabled_charts_to_update: &HashSet<ChartKey>,
        from: Option<NaiveDate>,
        update_later: bool,
        default_schedule: &Schedule,
    ) -> HashSet<ChartKey> {
        let Some((the_best_matching_group, enabled_members_to_update)) =
            self.choose_the_best_matching_group(enabled_charts_to_update)
        else {
            // no update groups
            return HashSet::new();
        };
        tracing::info!(
            group = the_best_matching_group.group.name(),
            requested_enabled_members =? enabled_members_to_update,
            "chosen next group to reupdate on-demand"
        );

        if let Some(reupdate_from) = from {
            self.set_next_update_from(
                reupdate_from,
                the_best_matching_group,
                &enabled_members_to_update,
            )
            .await;
        }
        if update_later {
            let group_schedule = group_update_schedule(the_best_matching_group, default_schedule);
            let next_update = time_till_next_call(group_schedule);
            tracing::info!(
                group = the_best_matching_group.group.name(),
                "Will update later according to group's schedule (in {next_update:?})"
            );
        } else {
            tracing::info!(
                group = the_best_matching_group.group.name(),
                "Updating the group right now on-demand"
            );
            self.clone()
                .update(
                    the_best_matching_group.clone(),
                    false,
                    Some(&enabled_members_to_update),
                )
                .await;
            tracing::info!(
                group = the_best_matching_group.group.name(),
                updated_members =? enabled_members_to_update,
                "successfully updated the group on-demand"
            );
        };
        enabled_members_to_update
    }

    fn choose_the_best_matching_group(
        &self,
        member_charts_to_update: &HashSet<ChartKey>,
    ) -> Option<(&UpdateGroupEntry, HashSet<ChartKey>)> {
        self.charts
            .update_groups
            .values()
            .map(|g| {
                (
                    g,
                    g.enabled_members
                        .intersection(member_charts_to_update)
                        .count(),
                )
            })
            .max_by_key(|(_, members_to_update)| *members_to_update)
            .map(|(g, _)| {
                (
                    g,
                    member_charts_to_update
                        .intersection(&g.enabled_members)
                        .cloned()
                        .collect(),
                )
            })
    }

    async fn set_next_update_from(
        &self,
        from: NaiveDate,
        group_entry: &UpdateGroupEntry,
        enabled_charts_to_update: &HashSet<ChartKey>,
    ) {
        let result = group_entry
            .group
            .set_next_update_from_sync(&self.db, from, enabled_charts_to_update)
            .await;
        if let Err(err) = result {
            tracing::error!(
                update_group = group_entry.group.name(),
                "error setting next update from: {}",
                err
            );
        } else {
            tracing::info!(
                update_group = group_entry.group.name(),
                "successfully set next update from (will update from {})",
                from
            );
        }
    }

    async fn run_recurrent_update(
        self: &Arc<Self>,
        group_entry: UpdateGroupEntry,
        schedule: Schedule,
    ) {
        let this = self.clone();
        let chart = group_entry.clone();
        this.run_cron(chart, schedule).await
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
            self.clone().update(group_entry.clone(), false, None).await;
        }
    }

    async fn update(
        self: Arc<Self>,
        group_entry: UpdateGroupEntry,
        force_full: bool,
        enabled_charts_overwrite: Option<&HashSet<ChartKey>>,
    ) {
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
        let enabled_charts = enabled_charts_overwrite.unwrap_or(&group_entry.enabled_members);
        let update_parameters = UpdateParameters {
            db: &self.db,
            blockscout: &self.blockscout_db,
            blockscout_applied_migrations: active_migrations,
            enabled_update_charts_recursive: group_entry
                .group
                .enabled_members_with_deps(enabled_charts),
            update_time_override: None,
            force_full,
        };
        let result = group_entry
            .group
            .update_charts_sync(update_parameters, enabled_charts)
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

    pub async fn handle_update_request(
        self: &Arc<Self>,
        chart_names: Vec<String>,
        from: Option<NaiveDate>,
        update_later: bool,
    ) -> Result<OnDemandReupdateAccepted, OnDemandReupdateError> {
        let (accepted_keys, accepted_names, rejections) =
            self.split_update_request_input(chart_names);
        if accepted_keys.is_empty() {
            return Err(OnDemandReupdateError::AllChartsNotFound);
        }

        self.on_demand_sender
            .lock()
            .await
            .send(OnDemandReupdateRequest {
                chart_names: accepted_keys,
                from,
                update_later,
            })
            .await
            .map_err(|_| {
                tracing::error!("on demand channel closed");
                OnDemandReupdateError::Internal
            })?;
        Ok(OnDemandReupdateAccepted {
            accepted: accepted_names,
            rejected: rejections,
        })
    }

    pub async fn get_initial_update_status(&self) -> proto_v1::UpdateStatus {
        proto_v1::UpdateStatus {
            all_status: self.init_update_tracker.get_all_status().await.into(),
            independent_status: self
                .init_update_tracker
                .get_independent_status()
                .await
                .into(),
            blocks_dependent_status: self
                .init_update_tracker
                .get_blocks_dependent_status()
                .await
                .into(),
            internal_transactions_dependent_status: self
                .init_update_tracker
                .get_internal_transactions_dependent_status()
                .await
                .into(),
            user_ops_dependent_status: self
                .init_update_tracker
                .get_user_ops_dependent_status()
                .await
                .into(),
        }
    }

    pub fn initial_update_tracker(&self) -> &InitialUpdateTracker {
        &self.init_update_tracker
    }

    /// (accepted_chart_keys, accepted_chart_names, rejected_chart_names)
    fn split_update_request_input(
        self: &Arc<Self>,
        chart_names: Vec<String>,
    ) -> (HashSet<ChartKey>, Vec<String>, Vec<Rejection>) {
        let (found, not_found): (Vec<_>, Vec<_>) = chart_names
            .into_iter()
            .map(|name| {
                if let Some(entry) = self.charts.charts_info.get(&name) {
                    Ok((name, entry.get_keys()))
                } else {
                    Err(name)
                }
            })
            .partition_result();
        let rejections = not_found
            .into_iter()
            .map(|name| Rejection {
                name,
                reason: "chart name was not found".to_string(),
            })
            .collect();
        let (accepted_names, accepted_keys): (Vec<_>, Vec<_>) = found.into_iter().unzip();
        let accepted_keys: HashSet<_> = accepted_keys.into_iter().flatten().collect();
        (accepted_keys, accepted_names, rejections)
    }
}

#[derive(Clone, Debug)]
struct OnDemandReupdateRequest {
    pub chart_names: HashSet<ChartKey>,
    pub from: Option<NaiveDate>,
    pub update_later: bool,
}

#[derive(Error, Debug)]
pub enum OnDemandReupdateError {
    #[error("All provided chart names were not found")]
    AllChartsNotFound,
    #[error("internal error")]
    Internal,
}

pub struct OnDemandReupdateAccepted {
    pub accepted: Vec<String>,
    pub rejected: Vec<Rejection>,
}

impl OnDemandReupdateAccepted {
    pub fn into_update_result(self) -> proto_v1::BatchUpdateChartsResult {
        proto_v1::BatchUpdateChartsResult {
            total: (self.accepted.len() + self.rejected.len()) as u32,
            total_rejected: self.rejected.len() as u32,
            accepted: self.accepted,
            rejected: self
                .rejected
                .into_iter()
                .map(|r| proto_v1::BatchUpdateChartRejection {
                    name: r.name,
                    reason: r.reason,
                })
                .collect(),
        }
    }
}

pub struct Rejection {
    pub name: String,
    pub reason: String,
}
