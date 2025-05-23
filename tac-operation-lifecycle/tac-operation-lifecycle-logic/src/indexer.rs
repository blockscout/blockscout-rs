use crate::{
    client::models::profiling::BlockchainType,
    database::{OrderDirection, TacDatabase},
    settings::IndexerSettings,
};
use anyhow::Error;
use client::{
    models::profiling::{OperationType, StageType},
    Client,
};
use futures::{
    stream::{select, select_with_strategy, BoxStream, PollNext},
    StreamExt,
};
use std::{cmp::max, collections::HashMap, fmt, str::FromStr, sync::Arc, time::Duration};
use tac_operation_lifecycle_entity::{interval, operation, sea_orm_active_enums::StatusEnum};
use tokio::{task::JoinHandle, time};
use tracing::{instrument, Instrument};
use uuid::Uuid;

use crate::client;

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

impl OperationType {
    pub fn to_id(&self) -> i32 {
        match self {
            OperationType::Pending => 1,
            OperationType::TonTacTon => 2,
            OperationType::TacTon => 3,
            OperationType::TonTac => 4,
            OperationType::Rollback => 5,
            OperationType::Unknown => 6,
            OperationType::ErrorType => 0,
        }
    }

    pub fn is_finalized(&self) -> bool {
        self != &OperationType::Pending && self != &OperationType::Unknown
    }
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SCREAMING_SNAKE_CASE
        let s = serde_json::to_string(self)
            .unwrap()
            .trim_matches('"')
            .to_string();

        write!(f, "{}", s)
    }
}

impl FromStr for OperationType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(serde_json::from_str(&format!("\"{}\"", s)).unwrap_or(OperationType::ErrorType))
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
        write!(f, "{:?}", self)
    }
}

impl fmt::Display for BlockchainType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
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
            Ok(operations_map) => {
                let mut processed_operations = 0;
                let mut completed_operations = 0;
                let forever_pending_operation_cap = (chrono::Utc::now()
                    - self.settings.forever_pending_operations_age_sec)
                    .timestamp();
                for (op_id, operation_data) in operations_map.iter() {
                    // Find an associated operation in the input operations vector
                    match jobs.iter().find(|j| &j.operation.id == op_id) {
                        Some(job) => {
                            let _ = self
                                .database
                                .set_operation_data(&job.operation, operation_data)
                                .await;

                            let new_status = if operation_data.operation_type.is_finalized() {
                                // The case when operation has a finalized status
                                // otherwise they will be catched by operation stream
                                StatusEnum::Completed
                            } else if operation_data.operation_type == OperationType::Pending
                                && job.operation.timestamp.and_utc().timestamp()
                                    < forever_pending_operation_cap
                            {
                                // The operations whitch remains PENDING after forever_pending_operations_age_sec
                                // are considered to be forever pending. We shouldn't recheck them anymore
                                StatusEnum::Completed
                            } else {
                                StatusEnum::Pending
                            };

                            let _ = (self
                                .database
                                .set_operation_status(&job.operation, &new_status))
                            .await;

                            processed_operations += 1;
                            if new_status == StatusEnum::Completed {
                                completed_operations += 1;
                            }
                        }
                        None => {
                            tracing::error!(
                                operation_id =? op_id,
                                "Stage profiling response contains unknown operation. Skipping..."
                            );
                        }
                    }
                }

                tracing::info!(
                    processed =? processed_operations,
                    completed =? completed_operations,
                    "Successfully processed operations: [{}]",
                    op_ids.join(", ")
                );
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
