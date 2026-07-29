// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{
    client::models::profiling::BlockchainType,
    database::{OrderDirection, TacDatabase},
    settings::IndexerSettings,
};
use anyhow::Error;
use client::{
    models::profiling::{LegacyOperationType, ProfilingResponse, SourceOperationData, StageType},
    settings::StageProfilingMode,
    Client,
};
use futures::{
    stream::{select, select_with_strategy, BoxStream, PollNext},
    StreamExt,
};
use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    fmt,
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use tac_operation_lifecycle_entity::{interval, operation, sea_orm_active_enums::StatusEnum};
use tokio::{task::JoinHandle, time};
use tracing::{instrument, Instrument};
use uuid::Uuid;

use crate::client;

const V2_BACKFILL_MIN_IDLE_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct Job {
    pub interval: interval::Model,
}

#[derive(Debug, Clone)]
pub struct OperationJob {
    pub operation: operation::Model,
}

#[derive(Debug, Clone)]
pub enum IndexerJob {
    Interval(Job),           // to request operation IDs within interval
    Operation(OperationJob), // to request profiling data for the operations
}

impl IndexerJob {
    pub fn job_type(&self) -> String {
        match self {
            IndexerJob::Interval(_) => "IntervalJob".to_string(),
            IndexerJob::Operation(_) => "OperationJob".to_string(),
        }
    }
}

impl LegacyOperationType {
    pub fn to_id(&self) -> i32 {
        match self {
            LegacyOperationType::Pending => 1,
            LegacyOperationType::TonTacTon => 2,
            LegacyOperationType::TacTon => 3,
            LegacyOperationType::TonTac => 4,
            LegacyOperationType::Rollback => 5,
            LegacyOperationType::Unknown => 6,
            LegacyOperationType::InsufficientFee => 7,
            LegacyOperationType::ErrorType => 0,
        }
    }

    pub fn is_finalized(&self) -> bool {
        match self {
            LegacyOperationType::Pending
            | LegacyOperationType::Unknown
            | LegacyOperationType::InsufficientFee => false,
            LegacyOperationType::TonTacTon
            | LegacyOperationType::TacTon
            | LegacyOperationType::TonTac
            | LegacyOperationType::Rollback
            | LegacyOperationType::ErrorType => true,
        }
    }
}

impl fmt::Display for LegacyOperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SCREAMING_SNAKE_CASE
        let s = serde_json::to_string(self)
            .unwrap()
            .trim_matches('"')
            .to_string();

        write!(f, "{s}")
    }
}

impl FromStr for LegacyOperationType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(serde_json::from_str(&format!("\"{s}\"")).unwrap_or(LegacyOperationType::ErrorType))
    }
}

impl StageType {
    pub fn to_id(&self) -> i32 {
        match self {
            StageType::CollectedInTAC => 1,
            StageType::IncludedInTACConsensus => 2,
            StageType::ExecutedInTAC => 3,
            StageType::CollectedInTON => 4,
            StageType::IncludedInTONConsensus => 5,
            StageType::ExecutedInTON => 6,
        }
    }
}

impl fmt::Display for StageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl fmt::Display for BlockchainType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

pub struct Indexer {
    settings: IndexerSettings,
    // This boundary timestamp will used for distinguish between historical and realtime intervals
    // The main difference lays within interval processing
    //  - the historical intervals are fetched by selected boundaries
    //  - the realtime intervals are always fetched FROM the realtime boundary
    //    (to avoid remote service sync issues)
    // The boundary can be updated on inserting new operations
    realtime_boundary: u64,
    database: Arc<TacDatabase>,
    client: Arc<Client>,
}

impl Indexer {
    pub async fn new(
        settings: IndexerSettings,
        db: Arc<TacDatabase>,
        client: Arc<Client>,
    ) -> anyhow::Result<Self> {
        const REALTIME_LAG_MINUTES: i64 = 10;
        // realtime boundary evaluation: few minutes before (to avoid remote service sync issues)
        let realtime_boundary_hard = (chrono::Utc::now()
            - chrono::Duration::minutes(REALTIME_LAG_MINUTES))
        .timestamp() as u64;
        // realtime boundary evaluation: last operation timestamp from the database
        let relatime_boundary_db = match db.get_latest_operation_timestamp().await {
            Ok(Some(ts)) => ts,
            _ => 0,
        };

        tracing::info!(
            realtime_boundary_hard =? realtime_boundary_hard,
            relatime_boundary_db =? relatime_boundary_db,
            "Indexer created"
        );

        Ok(Self {
            settings,
            realtime_boundary: max(realtime_boundary_hard, relatime_boundary_db),
            database: db,
            client,
        })
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::client::models::profiling::{
        OperationRoute, OperationStatus, V1OperationData, V2OperationData,
    };

    fn operation(timestamp: chrono::NaiveDateTime) -> operation::Model {
        operation::Model {
            id: "operation".to_string(),
            op_type: None,
            profiling_version: 1,
            op_status: None,
            finalized: None,
            rollback: None,
            timestamp,
            next_retry: None,
            status: StatusEnum::Pending,
            retry_count: 0,
            inserted_at: timestamp,
            updated_at: timestamp,
            sender_address: None,
            sender_blockchain: None,
        }
    }

    fn v1_data() -> V1OperationData {
        V1OperationData {
            operation_type: LegacyOperationType::Pending,
            meta_info: None,
            stages: HashMap::new(),
        }
    }

    fn v2_data(finalized: bool) -> V2OperationData {
        V2OperationData {
            operation_type: OperationRoute::TonTac,
            status: Some(OperationStatus::Failed),
            finalized,
            rollback: true,
            meta_info: None,
            stages: HashMap::new(),
        }
    }

    #[test]
    fn v1_partial_recovery_cannot_overwrite_v2_result() {
        let mut tagged = HashMap::from([
            (
                "v2-operation".to_string(),
                SourceOperationData::V2(v2_data(false)),
            ),
            (
                "other-operation".to_string(),
                SourceOperationData::V2(v2_data(true)),
            ),
        ]);
        let omitted = HashSet::from(["missing-operation"]);
        let v1 = HashMap::from([
            ("v2-operation".to_string(), v1_data()),
            ("missing-operation".to_string(), v1_data()),
        ]);

        Indexer::merge_v1_recovery_results(&mut tagged, &omitted, v1);

        assert!(matches!(
            tagged.get("v2-operation"),
            Some(SourceOperationData::V2(_))
        ));
        assert!(matches!(
            tagged.get("missing-operation"),
            Some(SourceOperationData::V1(_))
        ));
        assert_eq!(tagged.len(), 3);
    }

    #[test]
    fn v2_work_status_uses_finalized_and_preserves_forever_pending_semantics() {
        let settings = IndexerSettings {
            forever_pending_operations_age_sec: Duration::from_secs(60),
            ..Default::default()
        };
        let recent = operation(chrono::Utc::now().naive_utc());
        let old = operation((chrono::Utc::now() - chrono::Duration::minutes(2)).naive_utc());

        assert_eq!(
            Indexer::operation_work_status(
                &settings,
                &recent,
                &SourceOperationData::V2(v2_data(false))
            ),
            StatusEnum::Pending
        );
        assert_eq!(
            Indexer::operation_work_status(
                &settings,
                &recent,
                &SourceOperationData::V2(v2_data(true))
            ),
            StatusEnum::Completed
        );
        assert_eq!(
            Indexer::operation_work_status(
                &settings,
                &old,
                &SourceOperationData::V2(v2_data(false))
            ),
            StatusEnum::Completed
        );
    }

    #[test]
    fn backfill_convergence_is_reported_once_per_transition() {
        let mut convergence_reported = false;

        assert!(Indexer::should_report_v2_backfill_convergence(
            &mut convergence_reported,
            0
        ));
        assert!(!Indexer::should_report_v2_backfill_convergence(
            &mut convergence_reported,
            0
        ));
        assert!(!Indexer::should_report_v2_backfill_convergence(
            &mut convergence_reported,
            1
        ));
        assert!(Indexer::should_report_v2_backfill_convergence(
            &mut convergence_reported,
            0
        ));
    }

    #[test]
    fn empty_backfill_queue_uses_a_bounded_idle_interval() {
        let fast_settings = IndexerSettings {
            retry_interval: Duration::from_secs(1),
            ..Default::default()
        };
        let slow_settings = IndexerSettings {
            retry_interval: Duration::from_secs(120),
            ..Default::default()
        };

        assert_eq!(
            Indexer::v2_backfill_idle_interval(&fast_settings),
            Duration::from_secs(60)
        );
        assert_eq!(
            Indexer::v2_backfill_idle_interval(&slow_settings),
            Duration::from_secs(120)
        );
    }
}

impl Indexer {
    // Revert all intevals and operations in the 'processing' phase into the 'pending' one
    // This should be done on startup to avoid entities oblivion
    pub async fn reset_processing_operations(&self) -> anyhow::Result<(usize, usize)> {
        let intervals_affected = self.database.reset_processing_intervals().await?;
        let operations_affected = self.database.reset_processing_operations().await?;

        Ok((intervals_affected, operations_affected))
    }

    // Generate intervals between current epoch and watermark and save them to the db
    // Returns number of the generated intervals
    #[instrument(name = "generate_historical_intervals", skip_all, level = "info")]
    pub async fn generate_historical_intervals(&self, up_to: u64) -> anyhow::Result<usize> {
        let watermark = self.database.get_watermark().await?;
        let catchup_period = self.settings.catchup_interval.as_secs();
        let global_start = self.settings.start_timestamp;

        self.database
            .generate_pending_intervals(max(watermark, global_start), up_to, catchup_period)
            .await
    }

    pub async fn ensure_stages_types_exist(&self) -> Result<(), sea_orm::DbErr> {
        let stages_types = [
            StageType::CollectedInTAC,
            StageType::IncludedInTACConsensus,
            StageType::ExecutedInTAC,
            StageType::CollectedInTON,
            StageType::IncludedInTONConsensus,
            StageType::ExecutedInTON,
        ];

        let stages_map: HashMap<i32, String> = stages_types
            .iter()
            .map(|stage| (stage.to_id(), stage.to_string()))
            .collect();

        let _ = self.database.register_stage_types(&stages_map).await;

        Ok(())
    }

    pub async fn watermark(&self) -> anyhow::Result<u64> {
        self.database.get_watermark().await
    }

    fn create_realtime_thread(
        &self,
        initial_realtime_boundary: u64,
        polling_interval: Duration,
    ) -> JoinHandle<()> {
        let db = self.database.clone();
        let client = self.client.clone();

        tokio::spawn(async move {
            let mut realtime_bnd = initial_realtime_boundary;
            loop {
                let now = chrono::Utc::now().timestamp() as u64;

                tracing::info!(period =? (now - realtime_bnd), from =? realtime_bnd, to =? now, "Requesting for realtime operations");

                match client.get_operations(realtime_bnd, now).await {
                    Ok(operations) => {
                        if !operations.is_empty() {
                            tracing::info!(
                                count =? operations.len(),
                                period_sec = now - realtime_bnd,
                                "Fetched REALTIME operations: [{}]",
                                operations
                                    .iter()
                                    .map(|o| format!("{} [{}]", o.id.clone(), o.timestamp))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );

                            match db.insert_pending_operations(&operations).await {
                                Ok(_) => {
                                    let max_timestamp =
                                        operations.iter().map(|o| o.timestamp).max().unwrap();
                                    if max_timestamp > realtime_bnd {
                                        let _ = db
                                            .add_completed_interval(realtime_bnd, max_timestamp)
                                            .await;

                                        realtime_bnd = max_timestamp + 1;
                                        tracing::info!(
                                            new_value =? realtime_bnd,
                                            "Realtime boundary is moved forward"
                                        );
                                    }
                                }

                                Err(e) => {
                                    tracing::error!(
                                        err =? e,
                                        count =? operations.len(),
                                        "Failed to store realtime operations",
                                    );
                                }
                            }
                        }
                    }

                    Err(e) => {
                        tracing::error!(err =? e, "Failed to fetch REALTIME interval");
                    }
                }

                time::sleep(polling_interval).await;
            }
        })
    }

    pub fn interval_stream(
        &self,
        direction: OrderDirection,
        from: Option<u64>,
        to: Option<u64>,
    ) -> BoxStream<'_, IndexerJob> {
        Box::pin(async_stream::stream! {
            loop {
                let span_id = Uuid::new_v4();
                match self.database.query_pending_intervals(self.settings.intervals_query_batch, direction, from, to)
                    .instrument(tracing::debug_span!(
                        "INTERVALS",
                        span_id = span_id.to_string(),
                        direction = direction.sql_order_string()
                    ))
                    .await {
                    Ok(selected) => {
                        for interval in selected {
                            // Yield the job
                            yield IndexerJob::Interval(Job {
                                interval: interval.clone(),
                            });
                        }
                    },
                    Err(e) => {
                        tracing::error!(
                            requested_count =? self.settings.intervals_query_batch,
                            from,
                            to,
                            direction =? direction,
                            err =? e,
                            "Unable to select intervals from the database"
                        );
                    },
                }

                // Sleep a bit before next iteration to prevent tight loop
                tokio::time::sleep(self.settings.intervals_loop_delay_ms).await;
            }
        })
    }

    pub fn new_operations_stream(&self) -> BoxStream<'_, IndexerJob> {
        Box::pin(async_stream::stream! {
            loop {
                let span_id = Uuid::new_v4();
                match self.database.query_new_operations(self.settings.operations_query_batch, OrderDirection::LatestFirst)
                    .instrument(tracing::debug_span!(
                        "NEW OPERATIONS",
                        span_id = span_id.to_string()
                    ))
                    .await {
                    Ok(selected) => {
                        for operation in selected {
                            // Yield the job
                            yield IndexerJob::Operation(OperationJob { operation });
                        }
                    },
                    Err(e) => {
                        tracing::error!(
                            err =? e,
                            "Unable to select latest operations from the database"
                        );
                    },
                }

                // Sleep a bit before next iteration to prevent tight loop
                tokio::time::sleep(self.settings.operations_loop_delay_ms).await;
            }
        })
    }

    pub fn pending_operations_stream(&self) -> BoxStream<'_, IndexerJob> {
        Box::pin(async_stream::stream! {
            loop {
                let span_id = Uuid::new_v4();
                match self.database.query_pending_operations(self.settings.operations_query_batch, OrderDirection::LatestFirst)
                    .instrument(tracing::debug_span!(
                        "PENDING OPERATIONS",
                        span_id = span_id.to_string()
                    ))
                    .await {
                    Ok(selected) => {
                        for operation in selected {
                            // Yield the job
                            yield IndexerJob::Operation(OperationJob { operation });
                        }
                    },
                    Err(e) => {
                        tracing::error!(
                            err =? e,
                            "Unable to select latest operations from the database"
                        );
                    },
                }

                // Sleep a bit before next iteration to prevent tight loop
                tokio::time::sleep(self.settings.operations_loop_delay_ms).await;
            }
        })
    }

    pub fn retry_intervals_stream(&self) -> BoxStream<'_, IndexerJob> {
        Box::pin(async_stream::stream! {
            loop {
                let span_id = Uuid::new_v4();
                match self.database.query_failed_intervals(self.settings.intervals_retry_batch)
                    .instrument(tracing::debug_span!(
                        "FAILED INTERVALS",
                        span_id = span_id.to_string()
                    ))
                    .await {
                    Ok(selected) => {
                        if !selected.is_empty() {
                            tracing::info!(count =? selected.len(), "Found failed intervals");
                            for interval in selected {
                                // Yield the job
                                yield IndexerJob::Interval(Job {
                                    interval: interval.clone(),
                                });
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!(err =? e, "Unable to select failed intervals from the database");
                    },
                }

                tokio::time::sleep(self.settings.retry_interval).await;
            }
        })
    }

    pub fn retry_operations_stream(&self) -> BoxStream<'_, IndexerJob> {
        Box::pin(async_stream::stream! {
            loop {
                let span_id = Uuid::new_v4();
                match self.database.query_failed_operations(self.settings.operations_retry_batch, OrderDirection::EarliestFirst)
                    .instrument(tracing::debug_span!(
                        "FAILED OPERATIONS",
                        span_id = span_id.to_string()
                    ))
                    .await {
                    Ok(selected) => {
                        if !selected.is_empty() {
                            tracing::info!(count =? selected.len(), "Found failed operations");
                            for operation in selected {
                                // Yield the job
                                yield IndexerJob::Operation(OperationJob { operation });
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!(err =? e, "Unable to select failed operations from the database");
                    },
                }

                tokio::time::sleep(self.settings.retry_interval).await;
            }
        })
    }

    pub async fn process_interval_with_retries(&self, job: &Job) {
        match self
            .fetch_historical_operations(job)
            .instrument(tracing::debug_span!("fetching operations for interval",))
            .await
        {
            Ok(num) => {
                if num > 0 {
                    tracing::debug!(
                        interval_id =? job.interval.id,
                        count =? num,
                        "Successfully fetched interval",
                    );
                }
            }

            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch interval");

                let attempt = job.interval.retry_count + 1;
                let base_delay = 5; // 5 seconds base delay
                let next_retry_after = base_delay * attempt as i64;

                let _ = self
                    .database
                    .set_interval_retry(&job.interval, next_retry_after)
                    .await;
            }
        }
    }

    pub async fn fetch_historical_operations(&self, job: &Job) -> Result<usize, Error> {
        tracing::debug!(
            job =? job,
            "Processing interval job",
        );

        let request_start = job.interval.start.and_utc().timestamp() as u64;
        let request_end = job.interval.finish.and_utc().timestamp() as u64;
        let operations = self
            .client
            .get_operations(request_start + 1, request_end)
            .instrument(tracing::debug_span!(
                "get_operations",
                interval_id = job.interval.id,
                start = request_start,
                finish = request_end,
            ))
            .await?;
        let ops_num = operations.len();

        if ops_num > 0 {
            tracing::info!(
                count =? ops_num,
                period_sec = request_end - request_start,
                "Fetched HISTORICAL operations: [{}]",
                operations
                    .iter()
                    .map(|o| o.id.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            self.database.insert_pending_operations(&operations).await?;
        }

        self.database
            .set_interval_status(&job.interval, &StatusEnum::Completed)
            .await?;

        tracing::debug!(
            interval_id =? job.interval.id,
            "Successfully processed job",
        );

        Ok(ops_num)
    }

    pub async fn process_operation_with_retries(&self, jobs: Vec<&OperationJob>) {
        let op_ids: Vec<&str> = jobs.iter().map(|j| j.operation.id.as_str()).collect();

        match self.client.get_operations_stages(op_ids.clone()).await {
            Ok(response) => {
                let mut values: HashMap<String, SourceOperationData> = match response {
                    ProfilingResponse::V1(values) => values
                        .into_iter()
                        .map(|(id, data)| (id, SourceOperationData::V1(data)))
                        .collect(),
                    ProfilingResponse::V2(values) => {
                        let mut tagged: HashMap<_, _> = values
                            .into_iter()
                            .map(|(id, data)| (id, SourceOperationData::V2(data)))
                            .collect();
                        if self.client.stage_profiling_mode() == StageProfilingMode::PreferV2 {
                            let omitted: Vec<_> = op_ids
                                .iter()
                                .copied()
                                .filter(|id| !tagged.contains_key(*id))
                                .collect();
                            if !omitted.is_empty() {
                                let omitted: HashSet<_> = omitted.into_iter().collect();
                                match self
                                    .client
                                    .get_operations_stages_v1(omitted.iter().copied().collect())
                                    .await
                                {
                                    Ok(v1) => {
                                        Self::merge_v1_recovery_results(&mut tagged, &omitted, v1)
                                    }
                                    Err(error) => tracing::warn!(
                                        %error,
                                        "Failed to recover IDs omitted by Stage Profiler v2"
                                    ),
                                }
                            }
                        }
                        tagged
                    }
                };
                self.persist_operation_results(&jobs, &op_ids, &mut values)
                    .await;
            }
            Err(e) => {
                tracing::error!(
                    err =? e,
                    count =? op_ids.len(),
                    jobs_cnt =? jobs.len(),
                    "Failed to fetch operations: [{}]",
                    op_ids.join(", ")
                );

                for job in jobs {
                    let attempt = job.operation.retry_count + 1;
                    let base_delay = 5; // 5 seconds base delay
                    let next_retry_after = base_delay * attempt as i64;

                    let _ = self
                        .database
                        .set_operation_retry(&job.operation, next_retry_after)
                        .await;
                }
            }
        }
    }

    fn merge_v1_recovery_results(
        tagged: &mut HashMap<String, SourceOperationData>,
        omitted: &HashSet<&str>,
        v1: HashMap<String, client::models::profiling::V1OperationData>,
    ) {
        for (id, data) in v1 {
            if omitted.contains(id.as_str()) {
                tagged.insert(id, SourceOperationData::V1(data));
            } else {
                tracing::warn!(
                    operation_id = %id,
                    "Stage Profiler v1 recovery returned a non-omitted operation; skipping"
                );
            }
        }
    }

    async fn persist_operation_results(
        &self,
        jobs: &[&OperationJob],
        requested_ids: &[&str],
        values: &mut HashMap<String, SourceOperationData>,
    ) {
        let requested: HashSet<_> = requested_ids.iter().copied().collect();
        for extra in values.keys().filter(|id| !requested.contains(id.as_str())) {
            tracing::error!(
                operation_id = %extra,
                "Stage profiling response contains unknown operation; skipping"
            );
        }

        let mut processed = 0usize;
        let mut completed = 0usize;
        for job in jobs {
            let Some(operation_data) = values.remove(&job.operation.id) else {
                tracing::warn!(
                    operation_id = %job.operation.id,
                    "Stage profiling response omitted requested operation"
                );
                self.retry_operation(&job.operation).await;
                continue;
            };
            let new_status =
                Self::operation_work_status(&self.settings, &job.operation, &operation_data);
            if let Err(error) = self
                .database
                .set_operation_data(&job.operation, &operation_data, &new_status)
                .await
            {
                tracing::error!(
                    operation_id = %job.operation.id,
                    %error,
                    "Failed to store operation data"
                );
                self.retry_operation(&job.operation).await;
                continue;
            }
            processed += 1;
            completed += usize::from(new_status == StatusEnum::Completed);
        }
        tracing::info!(
            processed,
            completed,
            "Successfully processed operations: [{}]",
            requested_ids.join(", ")
        );
    }

    fn operation_work_status(
        settings: &IndexerSettings,
        operation: &operation::Model,
        data: &SourceOperationData,
    ) -> StatusEnum {
        let cap = (chrono::Utc::now() - settings.forever_pending_operations_age_sec).timestamp();
        let age_capped = operation.timestamp.and_utc().timestamp() < cap;
        match data {
            SourceOperationData::V2(data) if data.finalized => StatusEnum::Completed,
            SourceOperationData::V2(data) if age_capped => {
                tracing::warn!(
                    operation_id = %operation.id,
                    route = %data.operation_type,
                    op_status = ?data.status,
                    finalized = data.finalized,
                    rollback = data.rollback,
                    technical_status = "completed",
                    "Forever-pending operation reached the local polling stop"
                );
                StatusEnum::Completed
            }
            SourceOperationData::V2(_) => StatusEnum::Pending,
            SourceOperationData::V1(data) => {
                let operation_type =
                    crate::lifecycle::derive_v1_source_type(&data.operation_type, &data.stages);
                if operation_type.is_finalized() {
                    return StatusEnum::Completed;
                }
                if age_capped
                    && matches!(
                        operation_type,
                        LegacyOperationType::Pending | LegacyOperationType::InsufficientFee
                    )
                {
                    tracing::warn!(
                        operation_id = %operation.id,
                        op_type = %operation_type,
                        technical_status = "completed",
                        "Forever-pending operation reached the local polling stop"
                    );
                    StatusEnum::Completed
                } else {
                    StatusEnum::Pending
                }
            }
        }
    }

    fn should_report_v2_backfill_convergence(
        convergence_reported: &mut bool,
        remaining: u64,
    ) -> bool {
        if remaining == 0 {
            let should_report = !*convergence_reported;
            *convergence_reported = true;
            should_report
        } else {
            *convergence_reported = false;
            false
        }
    }

    fn v2_backfill_idle_interval(settings: &IndexerSettings) -> Duration {
        settings.retry_interval.max(V2_BACKFILL_MIN_IDLE_INTERVAL)
    }

    async fn retry_operation(&self, operation: &operation::Model) {
        let attempt = operation.retry_count + 1;
        let _ = self
            .database
            .set_operation_retry(operation, 5 * i64::from(attempt))
            .await;
    }

    fn create_v2_backfill_thread(&self) -> Option<JoinHandle<()>> {
        if self.client.stage_profiling_mode() == StageProfilingMode::V1Only {
            return None;
        }
        let database = self.database.clone();
        let client = self.client.clone();
        let settings = self.settings.clone();
        Some(tokio::spawn(async move {
            tracing::info!("Stage Profiler v2 re-profiling worker started");
            let mut convergence_reported = false;
            loop {
                if !client.v2_available_for_direct_request() {
                    tracing::debug!("V2 re-profiling paused while circuit is open");
                    time::sleep(
                        settings
                            .operations_loop_delay_ms
                            .max(Duration::from_secs(1)),
                    )
                    .await;
                    continue;
                }
                match database
                    .query_v1_operations_for_backfill(settings.operations_query_batch)
                    .await
                {
                    Ok(operations) if operations.is_empty() => {
                        client.release_v2_probe();
                        if let Ok(remaining) = database.count_v1_profiled_operations().await {
                            if remaining == 0 {
                                if Self::should_report_v2_backfill_convergence(
                                    &mut convergence_reported,
                                    remaining,
                                ) {
                                    tracing::info!("V2 re-profiling queue is converged");
                                }
                            } else {
                                Self::should_report_v2_backfill_convergence(
                                    &mut convergence_reported,
                                    remaining,
                                );
                                tracing::debug!(
                                    remaining,
                                    "V2 re-profiling has no currently claimable operations"
                                );
                            }
                        }
                        time::sleep(Self::v2_backfill_idle_interval(&settings)).await;
                        continue;
                    }
                    Ok(operations) => {
                        convergence_reported = false;
                        let ids: Vec<_> = operations.iter().map(|op| op.id.as_str()).collect();
                        match client.get_operations_stages_v2(ids).await {
                            Ok(mut response) => {
                                for operation in operations {
                                    match response.remove(&operation.id) {
                                        Some(data) => {
                                            let source = SourceOperationData::V2(data);
                                            let status = Self::operation_work_status(
                                                &settings, &operation, &source,
                                            );
                                            if let Err(error) = database
                                                .set_operation_data(&operation, &source, &status)
                                                .await
                                            {
                                                tracing::error!(
                                                    operation_id = %operation.id,
                                                    %error,
                                                    "Failed to store re-profiled operation"
                                                );
                                                let _ = database
                                                    .set_operation_retry(&operation, 5)
                                                    .await;
                                            }
                                        }
                                        None => {
                                            tracing::warn!(
                                                operation_id = %operation.id,
                                                "V2 backfill response omitted requested operation"
                                            );
                                            let _ =
                                                database.set_operation_retry(&operation, 5).await;
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                tracing::warn!(%error, "V2 re-profiling batch failed");
                                for operation in operations {
                                    let _ = database.set_operation_retry(&operation, 5).await;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        client.release_v2_probe();
                        tracing::error!(%error, "Unable to claim V2 re-profiling batch");
                    }
                }
                time::sleep(
                    settings
                        .operations_loop_delay_ms
                        .max(Duration::from_secs(1)),
                )
                .await;
            }
        }))
    }

    fn prio_left(_: &mut ()) -> PollNext {
        PollNext::Left
    }

    #[instrument(name = "indexer", skip_all, level = "info")]
    pub async fn start(&self) -> anyhow::Result<()> {
        tracing::info!("Initializing TAC indexer");

        self.ensure_stages_types_exist().await?;

        // Generate historical intervals
        let current_realtime_timestamp = self.realtime_boundary;

        let new_intervals = self
            .generate_historical_intervals(self.realtime_boundary)
            .await?;
        if new_intervals > 0 {
            tracing::info!(
                realtime_boundary =? self.realtime_boundary,
                intervals_count =? new_intervals,
                "Generated historical intervals"
            );
        }

        // Resetting intervals and operations status
        let (updated_intervals, updated_operations) = self.reset_processing_operations().await?;
        if updated_intervals > 0 {
            tracing::info!(
                count =? updated_intervals,
                "Found and reset intervals in 'processing' state"
            );
        }
        if updated_operations > 0 {
            tracing::info!(
                count =? updated_operations,
                "Found and reset operations in 'processing' state"
            );
        }

        let _v2_backfill_thread = self.create_v2_backfill_thread();

        // Start generating realtime intervals in the separated thread;
        let initial_realtime_boundary = self.database.get_watermark().await?;
        let realtime_thread =
            self.create_realtime_thread(initial_realtime_boundary, self.settings.polling_interval);

        // Create streams
        let realtime_intervals = self.interval_stream(
            OrderDirection::EarliestFirst,
            Some(current_realtime_timestamp),
            None,
        );
        let historical_intervals_high_priority = self.interval_stream(
            OrderDirection::LatestFirst,
            None,
            Some(current_realtime_timestamp),
        );
        let historical_intervals_low_priority = self.interval_stream(
            OrderDirection::EarliestFirst,
            None,
            Some(current_realtime_timestamp),
        );
        let pending_operations = self.pending_operations_stream();
        let new_operations = self.new_operations_stream();
        let failed_intervals = self.retry_intervals_stream();
        let failed_operations = self.retry_operations_stream();

        // Combine streams with prioritization (high priority first)
        let retry_stream = select(failed_intervals, failed_operations);
        let historical_intervals_stream = select_with_strategy(
            historical_intervals_high_priority,
            historical_intervals_low_priority,
            Self::prio_left,
        );
        let intervals_stream = select_with_strategy(
            realtime_intervals,
            historical_intervals_stream,
            Self::prio_left,
        );
        let combined_stream = select_with_strategy(
            select_with_strategy(pending_operations, new_operations, Self::prio_left),
            select_with_strategy(intervals_stream, retry_stream, Self::prio_left),
            Self::prio_left,
        );

        tracing::info!(forever_pending_hardcap =? self.settings.forever_pending_operations_age_sec, "NOTE: Old operations with PENDING type will considered as completed!");
        tracing::info!(current_realtime_timestamp, concurrency =? self.settings.concurrency, "Starting indexing stream...");

        combined_stream
            .for_each_concurrent(Some(self.settings.concurrency as usize), |job| async move {
                tracing::debug!("Getting a {}", job.job_type());
                match job {
                    IndexerJob::Interval(job) => {
                        self.process_interval_with_retries(&job)
                            .instrument(tracing::debug_span!("processing interval"))
                            .await;
                        anyhow::Ok(())
                    }
                    IndexerJob::Operation(job) => {
                        self.process_operation_with_retries([&job].to_vec())
                            .instrument(tracing::debug_span!("processing operation"))
                            .await;
                        anyhow::Ok(())
                    }
                }
                .unwrap_or_else(|err| {
                    tracing::error!(err =? err, "Failed to process job");
                })
            })
            .await;

        realtime_thread.await?;

        Ok(())
    }
}
