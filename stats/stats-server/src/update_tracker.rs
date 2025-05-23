use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use stats::{
    indexing_status::{BlockscoutIndexingStatus, IndexingStatusTrait, UserOpsIndexingStatus},
    ChartKey, IndexingStatus,
};
use stats_proto::blockscout::stats::v1 as proto_v1;
use tokio::sync::Mutex;

/// Tracks and reports progress of inital updates
pub struct InitialUpdateTracker {
    inner: Arc<Mutex<InitialUpdateTrackerInner>>,
}

impl InitialUpdateTracker {
    /// Need charts with their status requirements
    pub fn new(charts: &BTreeMap<ChartKey, IndexingStatus>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InitialUpdateTrackerInner::new(charts))),
        }
    }

    pub async fn get_all_charts_with_exact_status(
        &self,
        status: &proto_v1::ChartSubsetUpdateStatus,
    ) -> HashSet<ChartKey> {
        self.inner
            .lock()
            .await
            .get_all_charts_with_exact_status(status)
    }

    /// will skip tracking in a subset if not a part of the subset
    async fn mark_all_trackers(&self, charts: &HashSet<ChartKey>, status: UpdateStatusChange) {
        let mut inner = self.inner.lock().await;
        inner.mark_all_trackers(charts, status)
    }

    pub async fn mark_waiting_for_starting_condition(&self, charts: &HashSet<ChartKey>) {
        self.mark_all_trackers(charts, UpdateStatusChange::WaitingForStartingCondition)
            .await
    }

    pub async fn mark_queued_for_initial_update(&self, charts: &HashSet<ChartKey>) {
        self.mark_all_trackers(charts, UpdateStatusChange::QueuedForInitialUpdate)
            .await
    }

    pub async fn mark_started_initial_update(&self, charts: &HashSet<ChartKey>) {
        self.mark_all_trackers(charts, UpdateStatusChange::RunningInitialUpdate)
            .await
    }

    pub async fn mark_initial_update_done(&self, charts: &HashSet<ChartKey>) {
        self.mark_all_trackers(charts, UpdateStatusChange::CompletedInitialUpdate)
            .await
    }

    pub async fn get_independent_status(&self) -> proto_v1::ChartSubsetUpdateStatus {
        self.inner.lock().await.independent.get_status()
    }

    pub async fn get_blocks_dependent_status(&self) -> proto_v1::ChartSubsetUpdateStatus {
        self.inner.lock().await.blocks_dependent.get_status()
    }

    pub async fn get_internal_transactions_dependent_status(
        &self,
    ) -> proto_v1::ChartSubsetUpdateStatus {
        self.inner
            .lock()
            .await
            .internal_transactions_dependent
            .get_status()
    }

    pub async fn get_user_ops_dependent_status(&self) -> proto_v1::ChartSubsetUpdateStatus {
        self.inner.lock().await.user_ops_dependent.get_status()
    }

    pub async fn get_all_status(&self) -> proto_v1::ChartSubsetUpdateStatus {
        self.inner.lock().await.get_all_status()
    }

    /// Log progress
    pub async fn report(&self) {
        self.inner.lock().await.report();
    }
}

struct InitialUpdateTrackerInner {
    // each subset consists of a comprehensive set of enabled charts
    // that have the corresponding (or less strict) indexing status
    // requirement.
    // so, `internal_transactions_dependent` charts will include all
    // `blocks_dependent` charts, because internal txn dependency
    // implies blocks dependency.
    independent: UpdateChartSubsetTracker,
    blocks_dependent: UpdateChartSubsetTracker,
    internal_transactions_dependent: UpdateChartSubsetTracker,
    user_ops_dependent: UpdateChartSubsetTracker,
}

impl InitialUpdateTrackerInner {
    /// Need charts with their status requirements
    fn new(charts: &BTreeMap<ChartKey, IndexingStatus>) -> Self {
        let charts_satisfying_status =
            |charts: &BTreeMap<ChartKey, IndexingStatus>, status: &IndexingStatus| -> HashSet<_> {
                charts
                    .iter()
                    .filter(|(_, req)| status.is_requirement_satisfied(req))
                    .map(|(key, _)| key)
                    .cloned()
                    .collect()
            };

        let nothing_indexed_status = IndexingStatus {
            blockscout: BlockscoutIndexingStatus::NoneIndexed,
            user_ops: UserOpsIndexingStatus::IndexingPastOperations,
        };
        let only_blocks_indexed_status = IndexingStatus {
            blockscout: BlockscoutIndexingStatus::BlocksIndexed,
            user_ops: UserOpsIndexingStatus::IndexingPastOperations,
        };
        let internal_indexed_status = IndexingStatus {
            blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
            user_ops: UserOpsIndexingStatus::IndexingPastOperations,
        };
        let user_ops_indexed_status = IndexingStatus {
            // We want to include all user ops dependant charts
            // therefore we set blockscout to be as indexed as possible
            blockscout: BlockscoutIndexingStatus::MAX,
            user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
        };

        let independent = charts_satisfying_status(charts, &nothing_indexed_status);
        let blocks_dependent = charts_satisfying_status(charts, &only_blocks_indexed_status);
        let internal_transactions_dependent =
            charts_satisfying_status(charts, &internal_indexed_status);
        let user_ops_dependent = charts_satisfying_status(charts, &user_ops_indexed_status);
        Self::verify_tracking_all_charts(
            charts,
            &[
                &independent,
                &blocks_dependent,
                &internal_transactions_dependent,
                &user_ops_dependent,
            ],
        );
        InitialUpdateTrackerInner {
            independent: UpdateChartSubsetTracker::new(independent),
            blocks_dependent: UpdateChartSubsetTracker::new(blocks_dependent),
            internal_transactions_dependent: UpdateChartSubsetTracker::new(
                internal_transactions_dependent,
            ),
            user_ops_dependent: UpdateChartSubsetTracker::new(user_ops_dependent),
        }
    }

    fn get_all_charts_with_exact_status(
        &self,
        status: &proto_v1::ChartSubsetUpdateStatus,
    ) -> HashSet<ChartKey> {
        let all_trackers = [
            &self.independent,
            &self.blocks_dependent,
            &self.internal_transactions_dependent,
            &self.user_ops_dependent,
        ];
        all_trackers
            .into_iter()
            .flat_map(|tracker| tracker.get_charts_with_exact_status(status).clone())
            .collect()
    }

    fn verify_tracking_all_charts(
        charts: &BTreeMap<ChartKey, IndexingStatus>,
        tracking: &[&HashSet<ChartKey>],
    ) {
        let all_tracked = tracking
            .iter()
            .fold(HashSet::<&ChartKey>::new(), |mut all, next| {
                all.extend(next.iter());
                all
            });
        let charts = charts.keys().collect::<HashSet<_>>();
        let not_tracked: Vec<_> = charts.difference(&all_tracked).collect();
        if !not_tracked.is_empty() {
            tracing::warn!(
                "Some charts are not tracked for initial update ({not_tracked:?}). \
                The values returned in update status endpoint might be incorrect (overly optimistic). \
                This is a bug in service implementation. "
            )
        }
    }

    fn mark_all_trackers(&mut self, charts: &HashSet<ChartKey>, status: UpdateStatusChange) {
        for chart in charts {
            self.independent.track_status_change(chart, status.clone());
            self.blocks_dependent
                .track_status_change(chart, status.clone());
            self.internal_transactions_dependent
                .track_status_change(chart, status.clone());
            self.user_ops_dependent
                .track_status_change(chart, status.clone());
        }
    }

    fn joint_counts_by_status(&self) -> Vec<(String, usize)> {
        let mut counts = [0usize; 5];
        for subset in [
            &self.independent,
            &self.blocks_dependent,
            &self.internal_transactions_dependent,
            &self.user_ops_dependent,
        ] {
            let next_counts = subset.counts();
            for (c, n) in counts.iter_mut().zip(next_counts) {
                *c += n;
            }
        }
        vec![
            ("pending".to_string(), counts[0]),
            ("waiting_for_starting_condition".to_string(), counts[1]),
            ("queued_for_initial_update".to_string(), counts[2]),
            ("running_initial_update".to_string(), counts[3]),
            ("completed_initial_update".to_string(), counts[4]),
        ]
    }

    fn get_all_status(&self) -> proto_v1::ChartSubsetUpdateStatus {
        combine_statuses(&[
            self.independent.get_status(),
            self.blocks_dependent.get_status(),
            self.internal_transactions_dependent.get_status(),
            self.user_ops_dependent.get_status(),
        ])
    }

    fn report(&self) {
        let all_count_by_status = self.joint_counts_by_status();
        let mut log_string = String::new();
        for (status, count) in all_count_by_status {
            log_string.push_str(&format!("{status} charts - {count}; "));
        }
        let independent_status = self.independent.get_status();
        let blocks_dependent_status = self.blocks_dependent.get_status();
        let internal_transactions_dependent_status =
            self.internal_transactions_dependent.get_status();
        let user_ops_dependent_status = self.user_ops_dependent.get_status();
        let all_status = self.get_all_status();
        tracing::info!(
            independent_status =? independent_status,
            blocks_dependent_status =? blocks_dependent_status,
            internal_transactions_dependent_status =? internal_transactions_dependent_status,
            user_ops_dependent_status =? user_ops_dependent_status,
            all_status =? all_status,
            "update status report: {log_string}"
        );
    }
}

/// does not represent the proto encoding in any way;
/// it only shows the logical order of statuses
fn status_to_int(status: &proto_v1::ChartSubsetUpdateStatus) -> u32 {
    match status {
        proto_v1::ChartSubsetUpdateStatus::Pending => 0,
        proto_v1::ChartSubsetUpdateStatus::WaitingForStartingCondition => 1,
        proto_v1::ChartSubsetUpdateStatus::QueuedForInitialUpdate => 2,
        proto_v1::ChartSubsetUpdateStatus::RunningInitialUpdate => 3,
        proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate => 4,
    }
}

fn combine_statuses<'a>(
    statuses: impl IntoIterator<Item = &'a proto_v1::ChartSubsetUpdateStatus>,
) -> proto_v1::ChartSubsetUpdateStatus {
    let status = statuses
        .into_iter()
        .min_by(|a, b| status_to_int(a).cmp(&status_to_int(b)))
        .cloned();
    status.unwrap_or(proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate)
}

/// Track update status of some arbitrary chart set
struct UpdateChartSubsetTracker {
    // the charts travel from the topmost to the bottom field as
    // they get processed
    /// charts for which no actions were taken yet
    pending: HashSet<ChartKey>,
    waiting_for_starting_condition: HashSet<ChartKey>,
    queued_for_initial_update: HashSet<ChartKey>,
    running_initial_update: HashSet<ChartKey>,
    completed_initial_update: HashSet<ChartKey>,
}

#[derive(Debug, Clone)]
enum UpdateStatusChange {
    WaitingForStartingCondition,
    QueuedForInitialUpdate,
    RunningInitialUpdate,
    CompletedInitialUpdate,
}

impl UpdateChartSubsetTracker {
    pub fn new(charts: HashSet<ChartKey>) -> Self {
        Self {
            pending: charts,
            waiting_for_starting_condition: HashSet::new(),
            queued_for_initial_update: HashSet::new(),
            running_initial_update: HashSet::new(),
            completed_initial_update: HashSet::new(),
        }
    }

    pub fn track_status_change(&mut self, chart: &ChartKey, change: UpdateStatusChange) {
        let key_from_previous_status = match change {
            UpdateStatusChange::WaitingForStartingCondition => self.pending.take(chart),
            UpdateStatusChange::QueuedForInitialUpdate => self
                .waiting_for_starting_condition
                .take(chart)
                .or_else(|| self.pending.take(chart)),
            UpdateStatusChange::RunningInitialUpdate => self
                .queued_for_initial_update
                .take(chart)
                .or_else(|| self.waiting_for_starting_condition.take(chart))
                .or_else(|| self.pending.take(chart)),
            UpdateStatusChange::CompletedInitialUpdate => self
                .running_initial_update
                .take(chart)
                .or_else(|| self.queued_for_initial_update.take(chart))
                .or_else(|| self.waiting_for_starting_condition.take(chart))
                .or_else(|| self.pending.take(chart)),
        };
        if let Some(key) = key_from_previous_status {
            match change {
                UpdateStatusChange::WaitingForStartingCondition => {
                    self.waiting_for_starting_condition.insert(key);
                }
                UpdateStatusChange::QueuedForInitialUpdate => {
                    self.queued_for_initial_update.insert(key);
                }
                UpdateStatusChange::RunningInitialUpdate => {
                    self.running_initial_update.insert(key);
                }
                UpdateStatusChange::CompletedInitialUpdate => {
                    self.completed_initial_update.insert(key);
                }
            }
        }
    }

    pub fn get_status(&self) -> proto_v1::ChartSubsetUpdateStatus {
        match (
            self.pending.is_empty(),
            self.waiting_for_starting_condition.is_empty(),
            self.queued_for_initial_update.is_empty(),
            self.running_initial_update.is_empty(),
            self.completed_initial_update.is_empty(),
        ) {
            (true, true, true, true, true) | (true, true, true, true, false) => {
                proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate
            }
            (true, true, true, false, _) => proto_v1::ChartSubsetUpdateStatus::RunningInitialUpdate,
            (true, true, false, _, _) => proto_v1::ChartSubsetUpdateStatus::QueuedForInitialUpdate,
            (true, false, _, _, _) => {
                proto_v1::ChartSubsetUpdateStatus::WaitingForStartingCondition
            }
            (false, _, _, _, _) => proto_v1::ChartSubsetUpdateStatus::Pending,
        }
    }

    pub fn counts(&self) -> [usize; 5] {
        [
            self.pending.len(),
            self.waiting_for_starting_condition.len(),
            self.queued_for_initial_update.len(),
            self.running_initial_update.len(),
            self.completed_initial_update.len(),
        ]
    }

    pub fn get_charts_with_exact_status(
        &self,
        status: &proto_v1::ChartSubsetUpdateStatus,
    ) -> &HashSet<ChartKey> {
        match status {
            proto_v1::ChartSubsetUpdateStatus::Pending => &self.pending,
            proto_v1::ChartSubsetUpdateStatus::WaitingForStartingCondition => {
                &self.waiting_for_starting_condition
            }
            proto_v1::ChartSubsetUpdateStatus::QueuedForInitialUpdate => {
                &self.queued_for_initial_update
            }
            proto_v1::ChartSubsetUpdateStatus::RunningInitialUpdate => &self.running_initial_update,
            proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate => {
                &self.completed_initial_update
            }
        }
    }
}
