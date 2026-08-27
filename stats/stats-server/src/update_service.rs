// SPDX-License-Identifier: LicenseRef-Blockscout

use chrono::{NaiveDate, Utc};
use cron::Schedule;
use futures::{StreamExt, stream::FuturesUnordered};
use itertools::Itertools;
use sea_orm::{DatabaseConnection, DbErr};
use stats_proto::blockscout::stats::v1 as proto_v1;
use thiserror::Error;
use tokio::sync::{Mutex, Semaphore, mpsc};

use crate::{
    InitialUpdateTracker,
    blockscout_waiter::IndexingStatusListener,
    interchain_indexer_api::{
        InterchainIndexerApiClient, SliceCatchupVerdict, VerdictSource, resolve_verdict,
    },
    runtime_setup::{RuntimeSetup, UpdateGroupEntry},
    settings::Mode,
};
use stats::{
    ChartKey, InterchainFilter, InterchainFilterConfig,
    data_source::types::{IndexerMigrations, UpdateParameters},
    resolve_only_indexed_by_bridge,
};

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// Parameters for constructing [`UpdateService`].
/// Used to avoid passing too many arguments to [`UpdateService::new`].
pub struct UpdateServiceConfig {
    pub db: Arc<DatabaseConnection>,
    pub indexer_db: Arc<DatabaseConnection>,
    pub second_indexer_db: Option<Arc<DatabaseConnection>>,
    pub charts: Arc<RuntimeSetup>,
    pub status_listener: Option<IndexingStatusListener>,
    pub mode: Mode,
    pub multichain_filter: Option<Vec<u64>>,
    pub interchain_filter: InterchainFilterConfig,
    /// `None` when `STATS__INTERCHAIN_INDEXER_API_URL` is not set, or outside
    /// `Interchain` mode: the per-cycle catch-up check is then disabled.
    /// `Arc`, not a bare value: the same client also lives in
    /// `blockscout_waiter::InterchainCatchupSource` for the aggregator, and both
    /// need it for the life of the process.
    pub interchain_indexer_api: Option<Arc<InterchainIndexerApiClient>>,
}

pub struct UpdateService {
    db: Arc<DatabaseConnection>,
    indexer_db: Arc<DatabaseConnection>,
    second_indexer_db: Option<Arc<DatabaseConnection>>,
    mode: Mode,
    multichain_filter: Option<Vec<u64>>,
    interchain_filter: InterchainFilterConfig,
    interchain_indexer_api: Option<Arc<InterchainIndexerApiClient>>,
    charts: Arc<RuntimeSetup>,
    status_listener: Option<IndexingStatusListener>,
    init_update_tracker: InitialUpdateTracker,
    /// Per-group memory of whether the *last API-derived* interchain catch-up
    /// verdict for that group was incomplete (`true`) or complete (`false`).
    /// Absent entry ⇒ no verdict has been API-derived for that group yet.
    ///
    /// Used only to force one extra full rebuild on an observed `false → true`
    /// transition — see `resolve_interchain_preflight`. Keyed by group name:
    /// a transition belongs to the one group whose slice actually completed
    /// and must not be consumable by another group's cycle.
    interchain_verdict_was_incomplete: Mutex<HashMap<String, bool>>,
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

/// Everything this cycle needs to resolve about the interchain indexer: the
/// observability horizon merged into the filter, and the catch-up verdict for
/// the configured slice.
struct InterchainPreflight {
    filter: InterchainFilter,
    /// `false` ⇒ every chart in the group recomputes from the filtered floor
    /// this cycle.
    slice_catchup_complete: bool,
}

/// Resolves this cycle's effective `slice_catchup_complete` for `group_name`,
/// updating `state` (the per-group "was the last API-derived verdict
/// incomplete" memory) in place, and forcing one extra full rebuild on an
/// observed `false → true` transition of the verdict.
///
/// **Why this exists.** Trigger 1 (the verdict itself) forces a rebuild only
/// while the verdict is `false`; trigger 2 (the stored-floor check) only sees
/// floor *movement*. With per-chain decoupled catch-up, one chain can write
/// its entire remaining history *above* another chain's already-lower floor —
/// so the floor never moves — and only then mark itself complete. The verdict
/// then flips straight from `false` to `true` with no cycle in between where a
/// rebuild was forced, and those interior rows would never enter any line
/// chart. This is exactly Option B′ from
/// `.memory-bank/research/update-range-anchoring-and-backfill-detection.md`:
/// *"any design that detects backfill purely by watching the floor is
/// therefore ruled out"* — trigger 2 alone is that design.
///
/// This is memory *in addition to* the floor check, not a replacement for it:
/// `decisions.md` Q7 rejected memory only as a *replacement*, because a
/// process restart inside the flip window loses it. That residual gap still
/// exists here — a restart between the `false` cycle and the `true` cycle
/// still loses the interior fill — but the common case (the process stays up
/// across the transition) is now covered.
///
/// **Only a verdict actually derived from the API may update `state` or
/// consume a pending transition.** An unavailable or unconfigured API leaves
/// `state` untouched and returns `verdict.complete` as-is: acting on
/// `resolve_verdict`'s `complete = true` fallback here would silently consume
/// the pending transition without ever having observed the real `true`.
///
/// A free function (rather than an `UpdateService` method body) so the
/// transition — and the "non-API verdict never touches state" rule — are
/// testable without constructing a full `UpdateService`.
fn resolve_interchain_verdict_transition(
    state: &mut HashMap<String, bool>,
    group_name: &str,
    verdict: &SliceCatchupVerdict,
) -> bool {
    if verdict.source != VerdictSource::IndexerApi {
        return verdict.complete;
    }
    let was_incomplete = state.get(group_name).copied().unwrap_or(false);
    state.insert(group_name.to_owned(), !verdict.complete);
    if was_incomplete && verdict.complete {
        false // the transition: force one more rebuild this cycle
    } else {
        verdict.complete
    }
}

impl UpdateService {
    pub async fn new(config: UpdateServiceConfig) -> Result<Self, DbErr> {
        let on_demand = mpsc::channel(128);
        let init_update_tracker = Self::initialize_update_tracker(&config.charts);
        Ok(Self {
            db: config.db,
            indexer_db: config.indexer_db,
            second_indexer_db: config.second_indexer_db,
            mode: config.mode,
            multichain_filter: config.multichain_filter,
            interchain_filter: config.interchain_filter,
            interchain_indexer_api: config.interchain_indexer_api,
            charts: config.charts,
            status_listener: config.status_listener,
            init_update_tracker,
            interchain_verdict_was_incomplete: Mutex::new(HashMap::new()),
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
            // to not produce unnecessary logs
            if group_entry.should_skip_update() {
                return;
            }
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
                    tracing::warn!(
                        "on-demand update list was incorrectly filtered and prepared. this is likely a bug"
                    );
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
                    tracing::warn!(
                        "on-demand updated list does not intersect with enabled charts list. this is likely a bug"
                    );
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

    /// `None` ⇒ **skip** this group's update this cycle. The only cause is an
    /// unresolvable observability horizon, whose existing policy this preserves
    /// verbatim. An API failure never skips.
    async fn resolve_interchain_preflight(&self, group_name: &str) -> Option<InterchainPreflight> {
        if self.mode != Mode::Interchain {
            return Some(InterchainPreflight {
                filter: self.interchain_filter.with_horizon(None),
                slice_catchup_complete: true,
            });
        }

        // Resolve the observability horizon for this update cycle. Once per group
        // update, alongside the migrations probe — same shape, same failure handling.
        //
        // On error the group is SKIPPED rather than computed without the horizon:
        // the operator asked for the restriction, and computing without it would
        // write silently-too-large values under a fingerprint claiming they are
        // filtered. The next scheduled run retries.
        let filter = if !self.interchain_filter.include_unindexed_chains() {
            let horizon = resolve_only_indexed_by_bridge(
                &self.indexer_db,
                self.interchain_filter.bridge_ids(),
            )
            .await
            .inspect_err(|err| {
                tracing::error!("error resolving the interchain observability horizon: {err:?}")
            })
            .ok()?;
            // The startup log can only show the operator-configured half of the
            // filter; the horizon is not known until this read. Logging it here is
            // the only way an operator can confirm the scope actually applied.
            tracing::debug!(
                update_group = group_name,
                horizon =? horizon,
                "resolved the interchain observability horizon for this cycle"
            );
            self.interchain_filter.with_horizon(Some(horizon))
        } else {
            self.interchain_filter.with_horizon(None)
        };

        // The verdict probe is gated on `Mode::Interchain` alone — unlike the
        // horizon probe above, it does not also require
        // `!include_unindexed_chains()`, since the catch-up check is meaningful
        // regardless of whether the observability horizon restriction is enabled.
        let relevant_bridges = self.interchain_filter.bridge_ids().map(|ids| ids.to_vec());
        let relevant_chains = self.interchain_filter.relevant_chain_ids();
        let response = match self.interchain_indexer_api.as_ref() {
            Some(client) => Some(client.indexing_progress().await),
            None => None,
        };
        let raw_payload_len = response
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|items| items.len());
        if let Some(Err(err)) = &response {
            tracing::warn!(
                update_group = group_name,
                error =? err,
                "interchain indexing status unavailable; not forcing a rebuild on that account \
                 (the stored-floor check still applies)"
            );
        }

        let verdict = resolve_verdict(
            response,
            relevant_bridges.as_deref(),
            relevant_chains.as_ref(),
        );

        if verdict.source == VerdictSource::IndexerApi {
            if verdict.complete {
                tracing::debug!(
                    update_group = group_name,
                    slice_catchup_complete = true,
                    pairs_considered = verdict.pairs_considered,
                    source =? verdict.source,
                    "resolved the interchain catch-up verdict for this cycle"
                );
            } else {
                // Capped: `verdict.holding` is one entry per relevant pair still
                // catching up, and with a large bridge/chain configuration this
                // field is unbounded — it fires every cycle for a group that
                // stays incomplete for a while, so an uncapped list here can
                // dominate the log volume. `holding_total` keeps the true count
                // visible even when the sample is truncated.
                const MAX_LOGGED_HOLDING_PAIRS: usize = 20;
                let holding_total = verdict.holding.len();
                let holding_sample =
                    &verdict.holding[..holding_total.min(MAX_LOGGED_HOLDING_PAIRS)];
                tracing::warn!(
                    update_group = group_name,
                    pairs_considered = verdict.pairs_considered,
                    holding_total,
                    holding =? holding_sample,
                    "interchain slice is still catching up; rebuilding every chart in this \
                     group from the filtered floor this cycle"
                );
            }
            if verdict.pairs_considered == 0 && raw_payload_len.is_some_and(|n| n > 0) {
                tracing::warn!(
                    configured_bridge_ids =? relevant_bridges,
                    relevant_chains =? relevant_chains,
                    "the configured interchain filter selects no pair the indexer reports; the \
                     catch-up verdict is vacuously complete"
                );
            } else if raw_payload_len == Some(0) {
                // Distinguished from the branch above: this is a `200` whose
                // `items` array itself is empty, not "rows returned but none
                // relevant". `ChainIndexingProgress`'s fields are read with
                // `#[serde(default)]`, so an envelope rename (e.g. `items` ->
                // something else) silently deserializes to `vec![]` — `Ok(vec![])`
                // is indistinguishable, at the type level, from "the indexer
                // genuinely has nothing to report". Without this warn the
                // verdict would resolve to vacuously complete with only a
                // `debug!`, which is exactly the "silently a no-op" failure this
                // check exists to prevent.
                tracing::warn!(
                    update_group = group_name,
                    "the interchain indexer status response carried no rows at all; the \
                     catch-up verdict is vacuously complete. If pairs are actually configured \
                     upstream, this may mean the response envelope no longer matches what \
                     stats expects (check for an `items` key rename)"
                );
            }
        }

        // Force one extra full rebuild on the observed `false → true` transition
        // of the *API-derived* verdict for this group — see
        // `resolve_interchain_verdict_transition`'s doc comment for the full
        // justification (Option B′, the interior-fill gap it closes, and the
        // residual restart-window gap it does not).
        let slice_catchup_complete = {
            let mut previous_incomplete = self.interchain_verdict_was_incomplete.lock().await;
            let effective_complete = resolve_interchain_verdict_transition(
                &mut previous_incomplete,
                group_name,
                &verdict,
            );
            if !effective_complete && verdict.complete {
                tracing::info!(
                    update_group = group_name,
                    "interchain catch-up verdict transitioned from incomplete to complete; \
                     forcing one more full rebuild this cycle to pick up any interior fill \
                     that landed above the floor while the verdict was still incomplete"
                );
            }
            effective_complete
        };

        Some(InterchainPreflight {
            filter,
            slice_catchup_complete,
        })
    }

    async fn update(
        self: Arc<Self>,
        group_entry: UpdateGroupEntry,
        force_full: bool,
        enabled_charts_overwrite: Option<&HashSet<ChartKey>>,
    ) {
        let enabled_charts = enabled_charts_overwrite.unwrap_or(&group_entry.enabled_members);
        if group_entry.should_skip_update() {
            return;
        }
        tracing::info!(
            // instrumentation is inside `update_charts_with_mutexes`
            update_group = group_entry.group.name(),
            force_update = force_full,
            "updating group of charts"
        );
        let Ok(active_migrations) = IndexerMigrations::query_from_db(self.mode, &self.indexer_db)
            .await
            .inspect_err(|err| {
                tracing::error!("error during blockscout migrations detection: {:?}", err)
            })
        else {
            return;
        };

        let Some(preflight) = self
            .resolve_interchain_preflight(&group_entry.group.name())
            .await
        else {
            return;
        };

        let update_parameters = UpdateParameters {
            stats_db: &self.db,
            mode: self.mode,
            multichain_filter: self.multichain_filter.clone(),
            interchain_filter: preflight.filter,
            indexer_db: &self.indexer_db,
            second_indexer_db: self.second_indexer_db.as_deref(),
            indexer_applied_migrations: active_migrations,
            enabled_update_charts_recursive: group_entry
                .group
                .enabled_members_with_deps(enabled_charts),
            update_time_override: None,
            force_full: force_full || !preflight.slice_catchup_complete,
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

    /// `update_all=true` will ignore `chart_names` and update all enabled charts
    pub async fn handle_update_request(
        self: &Arc<Self>,
        mut chart_names: Vec<String>,
        update_all: bool,
        from: Option<NaiveDate>,
        update_later: bool,
    ) -> Result<OnDemandReupdateAccepted, OnDemandReupdateError> {
        if update_all {
            chart_names = self.charts.charts_info.keys().cloned().collect();
        }
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
        let tracker = &self.init_update_tracker;
        proto_v1::UpdateStatus {
            all_status: tracker.get_all_status().await.into(),
            independent_status: tracker.get_independent_status().await.into(),
            blocks_dependent_status: tracker.get_blocks_dependent_status().await.into(),
            internal_transactions_dependent_status: tracker
                .get_internal_transactions_dependent_status()
                .await
                .into(),
            user_ops_dependent_status: tracker.get_user_ops_dependent_status().await.into(),
            zetachain_cctx_dependent_status: tracker
                .get_zetachain_cctx_dependent_status()
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn verdict(source: VerdictSource, complete: bool) -> SliceCatchupVerdict {
        SliceCatchupVerdict {
            complete,
            pairs_considered: 0,
            holding: Vec::new(),
            source,
        }
    }

    /// The `false → true` transition: a group that was incomplete on the last
    /// API-derived cycle gets one forced rebuild the cycle it first reports
    /// complete, and the state then settles so a *second* consecutive complete
    /// cycle does not force again.
    #[test]
    fn interchain_verdict_transition_forces_one_rebuild_on_false_to_true() {
        let mut state = HashMap::new();
        let group = "test-group";

        // first cycle ever: nothing pending, verdict incomplete — no forcing,
        // remember incomplete
        assert!(!resolve_interchain_verdict_transition(
            &mut state,
            group,
            &verdict(VerdictSource::IndexerApi, false)
        ));
        assert_eq!(state.get(group), Some(&true));

        // still incomplete: no forcing needed, trigger 1 already covers it
        assert!(!resolve_interchain_verdict_transition(
            &mut state,
            group,
            &verdict(VerdictSource::IndexerApi, false)
        ));
        assert_eq!(state.get(group), Some(&true));

        // the transition: verdict reports complete after being incomplete —
        // force one more rebuild despite `verdict.complete == true`
        assert!(!resolve_interchain_verdict_transition(
            &mut state,
            group,
            &verdict(VerdictSource::IndexerApi, true)
        ));
        assert_eq!(state.get(group), Some(&false));

        // steady state complete: no more forcing
        assert!(resolve_interchain_verdict_transition(
            &mut state,
            group,
            &verdict(VerdictSource::IndexerApi, true)
        ));
        assert_eq!(state.get(group), Some(&false));
    }

    /// A verdict not derived from the API — unavailable or unconfigured — must
    /// never update the stored state, or it would consume a pending transition
    /// without ever having observed the real `true`.
    #[test]
    fn interchain_verdict_transition_ignores_non_api_verdicts() {
        let mut state = HashMap::new();
        let group = "test-group";

        assert!(!resolve_interchain_verdict_transition(
            &mut state,
            group,
            &verdict(VerdictSource::IndexerApi, false)
        ));
        assert_eq!(state.get(group), Some(&true));

        // an API outage between the incomplete cycle and the eventual complete
        // one must not consume or otherwise touch the pending transition
        assert!(resolve_interchain_verdict_transition(
            &mut state,
            group,
            &verdict(VerdictSource::ApiUnavailable, true)
        ));
        assert_eq!(
            state.get(group),
            Some(&true),
            "an unavailable-API verdict must not touch the stored state"
        );
        assert!(resolve_interchain_verdict_transition(
            &mut state,
            group,
            &verdict(VerdictSource::NotConfigured, true)
        ));
        assert_eq!(
            state.get(group),
            Some(&true),
            "a not-configured verdict must not touch the stored state either"
        );

        // the transition still fires once the API actually reports complete
        assert!(!resolve_interchain_verdict_transition(
            &mut state,
            group,
            &verdict(VerdictSource::IndexerApi, true)
        ));
        assert_eq!(state.get(group), Some(&false));
    }

    /// Two groups' transitions are independent: one group's completion must
    /// not consume another's pending transition.
    #[test]
    fn interchain_verdict_transition_is_per_group() {
        let mut state = HashMap::new();

        assert!(!resolve_interchain_verdict_transition(
            &mut state,
            "group-a",
            &verdict(VerdictSource::IndexerApi, false)
        ));
        assert!(!resolve_interchain_verdict_transition(
            &mut state,
            "group-b",
            &verdict(VerdictSource::IndexerApi, false)
        ));

        // group-a completes; group-b must still be pending afterwards
        assert!(!resolve_interchain_verdict_transition(
            &mut state,
            "group-a",
            &verdict(VerdictSource::IndexerApi, true)
        ));
        assert!(!resolve_interchain_verdict_transition(
            &mut state,
            "group-b",
            &verdict(VerdictSource::IndexerApi, true)
        ));
    }
}
