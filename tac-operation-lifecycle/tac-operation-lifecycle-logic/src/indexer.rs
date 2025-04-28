use std::{
    cmp::max,
    collections::HashMap,
    fmt,
    str::FromStr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::database::{OrderDirection, TacDatabase};
use anyhow::Error;
use client::{
    models::profiling::{BlockchainType, OperationType, StageType},
    Client,
};
use tac_operation_lifecycle_entity::{interval, operation, sea_orm_active_enums::StatusEnum};

use crate::settings::IndexerSettings;
use futures::{
    stream::{select, select_with_strategy, BoxStream, PollNext},
    StreamExt,
};
use tokio::sync::Mutex;
use tracing::{instrument, Instrument};
use uuid::Uuid;

use crate::client;

const INTERVALS_QUERY_RESULT_SIZE: usize = 1;
const INTERVALS_LOOP_DELAY_INTERVAL_MS: u64 = 100;
const OPERATIONS_QUERY_RESULT_SIZE: usize = 10;
const OPERATIONS_LOOP_DELAY_INTERVAL_MS: u64 = 200;

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
    Realtime,                // to generate realtime intervals
    Interval(Job),           // to request operation IDs within interval
    Operation(OperationJob), // to request profiling data for the operations
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
    realtime_boundary: AtomicU64,
    database: Arc<TacDatabase>,
    client: Arc<Mutex<Client>>,
}

impl Indexer {
    pub async fn new(
        settings: IndexerSettings,
        db: Arc<TacDatabase>,
        client: Arc<Mutex<Client>>,
    ) -> anyhow::Result<Self> {
        const REALTIME_LAG_MINUTES: i64 = 30;
        // realtime boundary evaluation: few minutes before (to avoid remote service sync issues)
        let realtime_boundary_hard = (chrono::Utc::now()
            - chrono::Duration::minutes(REALTIME_LAG_MINUTES))
        .timestamp() as u64;
        // realtime boundary evaluation: last operation timestamp from the database
        let relatime_boundary_db = match db.get_latest_operation_timestamp().await {
            Ok(Some(ts)) => ts,
            _ => 0,
        };
        Ok(Self {
            settings,
            realtime_boundary: AtomicU64::new(max(realtime_boundary_hard, relatime_boundary_db)),
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
    pub async fn generate_historical_intervals(&self, now: u64) -> anyhow::Result<usize> {
        let watermark = self.database.get_watermark().await?;
        let catchup_period = self.settings.catchup_interval.as_secs();
        let global_start = self.settings.start_timestamp;

        self.database
            .generate_pending_intervals(max(watermark, global_start), now, catchup_period)
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

    pub fn get_realtime_boundary(&self) -> u64 {
        self.realtime_boundary.load(Ordering::Acquire)
    }

    fn set_realtime_boundary(&self, new_boundary_candidate: u64) {
        // !the boundary cannot be set less than current
        let _ =
            self.realtime_boundary
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    if new_boundary_candidate > current {
                        tracing::info!(
                            new_value =? new_boundary_candidate,
                            "Realtime boundary is moved forward"
                        );
                        Some(new_boundary_candidate)
                    } else {
                        None
                    }
                });
    }

    pub fn realtime_stream(&self) -> BoxStream<'_, IndexerJob> {
        let interval_duration = self.settings.polling_interval;

        Box::pin(async_stream::stream! {
            loop {
                tokio::time::sleep(interval_duration).await;

                yield IndexerJob::Realtime;
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
                match self.database.query_pending_intervals(INTERVALS_QUERY_RESULT_SIZE, direction, from, to)
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
                            requested_count =? INTERVALS_QUERY_RESULT_SIZE,
                            from,
                            to,
                            direction =? direction,
                            err =? e,
                            "Unable to select intervals from the database"
                        );
                    },
                }

                // Sleep a bit before next iteration to prevent tight loop
                tokio::time::sleep(Duration::from_millis(INTERVALS_LOOP_DELAY_INTERVAL_MS)).await;
            }
        })
    }

    pub fn operations_stream(&self) -> BoxStream<'_, IndexerJob> {
        Box::pin(async_stream::stream! {
            loop {
                let span_id = Uuid::new_v4();
                match self.database.query_pending_operations(OPERATIONS_QUERY_RESULT_SIZE, OrderDirection::LatestFirst)
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
                tokio::time::sleep(Duration::from_millis(OPERATIONS_LOOP_DELAY_INTERVAL_MS)).await;
            }
        })
    }

    pub fn retry_intervals_stream(&self) -> BoxStream<'_, IndexerJob> {
        Box::pin(async_stream::stream! {
            loop {
                let span_id = Uuid::new_v4();
                match self.database.query_failed_intervals(INTERVALS_QUERY_RESULT_SIZE)
                    .instrument(tracing::debug_span!(
                        "FAILED INTERVALS",
                        span_id = span_id.to_string()
                    ))
                    .await {
                    Ok(selected) => {
                        tracing::debug!(count =? selected.len(), "Found failed intervals");
                        for interval in selected {
                            // Yield the job
                            yield IndexerJob::Interval(Job {
                                interval: interval.clone(),
                            });
                        }
                    },
                    Err(e) => {
                        tracing::error!(err =? e, "Unable to select failed intervals from the database");
                    },
                }

                // Sleep a bit before next iteration to prevent tight loop
                tokio::time::sleep(Duration::from_millis(INTERVALS_LOOP_DELAY_INTERVAL_MS)).await;
            }
        })
    }

    pub fn retry_operations_stream(&self) -> BoxStream<'_, IndexerJob> {
        const OPERATIONS_QUERY_RESULT_SIZE: usize = 1;
        const OPERATIONS_LOOP_DELAY_INTERVAL_MS: u64 = 2000;
        Box::pin(async_stream::stream! {
            loop {
                let span_id = Uuid::new_v4();
                match self.database.query_failed_operations(OPERATIONS_QUERY_RESULT_SIZE, OrderDirection::EarliestFirst)
                    .instrument(tracing::debug_span!(
                        "FAILED OPERATIONS",
                        span_id = span_id.to_string()
                    ))
                    .await {
                    Ok(selected) => {
                        tracing::debug!(count =? selected.len(), "Found failed operations");
                        for operation in selected {
                            // Yield the job
                            yield IndexerJob::Operation(OperationJob { operation });
                        }
                    },
                    Err(e) => {
                        tracing::error!(err =? e, "Unable to select failed operations from the database");
                    },
                }

                // Sleep a bit before next iteration to prevent tight loop
                tokio::time::sleep(Duration::from_millis(OPERATIONS_LOOP_DELAY_INTERVAL_MS)).await;
            }
        })
    }

    pub async fn process_interval_with_retries(&self, job: &Job) {
        match self
            .fetch_operations(job)
            .instrument(tracing::debug_span!("fetching operations for interval",))
            .await
        {
            Ok(num) => {
                if num > 0 {
                    tracing::debug!(
                        interval_id =? job.interval.id,
                        count =? num,
                        "Successfully fetched operations for interval",
                    );
                }
            }

            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch operations");

                // Schedule retry with exponential backoff
                let retries = job.interval.retry_count;
                let base_delay = 5; // 5 seconds base delay
                let next_retry_after = base_delay * 2i64.pow(retries as u32);

                let _ = self
                    .database
                    .set_interval_retry(&job.interval, next_retry_after)
                    .await;
            }
        }
    }

    pub async fn fetch_operations(&self, job: &Job) -> Result<usize, Error> {
        let mut client = self.client.lock().await;
        tracing::debug!(
            job =? job,
            "Processing interval job",
        );

        // Add helper function for timestamp conversion
        fn timestamp_to_naive(timestamp: i64) -> chrono::NaiveDateTime {
            chrono::DateTime::from_timestamp(timestamp, 0)
                .unwrap()
                .naive_utc()
        }
        let realtime_boundary = timestamp_to_naive(self.get_realtime_boundary() as i64);
        let (job_type, request_start) = if job.interval.start >= realtime_boundary {
            ("REALTIME", realtime_boundary)
        } else {
            ("HISTORICAL", job.interval.start)
        };

        let request_start = request_start.and_utc().timestamp() as u64;
        let request_end = job.interval.finish.and_utc().timestamp() as u64;
        let operations = client
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
                "Fetched {} operation_ids: [\n\t{}\n]",
                job_type,
                operations
                    .iter()
                    .map(|o| o.id.clone())
                    .collect::<Vec<_>>()
                    .join(",\n\t")
            );
            self.database.insert_pending_operations(&operations).await?;

            let max_timestamp = operations.iter().map(|o| o.timestamp).max().unwrap();
            self.set_realtime_boundary(max_timestamp);
        }

        self.database
            .set_interval_status(&job.interval, &StatusEnum::Completed)
            .await?;

        tracing::debug!(
            job_type,
            interval_id =? job.interval.id,
            "Successfully processed job",
        );

        Ok(ops_num)
    }

    pub async fn process_operation_with_retries(&self, jobs: Vec<&OperationJob>) {
        let mut client = self.client.lock().await;
        let op_ids: Vec<&str> = jobs.iter().map(|j| j.operation.id.as_str()).collect();

        match client.get_operations_stages(op_ids.clone()).await {
            Ok(operations_map) => {
                let mut processed_operations = 0;
                for (op_id, operation_data) in operations_map.iter() {
                    // Find an associated operation in the input operations vector
                    match jobs.iter().find(|j| &j.operation.id == op_id) {
                        Some(job) => {
                            let _ = self
                                .database
                                .set_operation_data(&job.operation, operation_data)
                                .await;

                            if operation_data.operation_type != OperationType::Pending {
                                // The case when operation has a finalized status
                                // otherwise they will be catched by operation stream
                                let _ = (self
                                    .database
                                    .set_operation_status(&job.operation, &StatusEnum::Completed))
                                .await;
                            }
                            processed_operations += 1;
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
                    count =? processed_operations,
                    "Successfully processed operations: [{}]",
                    op_ids.join(",\n\t")
                );
            }
            _ => {
                let _ = jobs.iter().map(|job| async {
                    let retries = job.operation.retry_count;
                    let base_delay = 5; // 5 seconds base delay
                    let next_retry_after = base_delay * 2i64.pow(retries as u32);

                    let _ = self
                        .database
                        .set_operation_retry(&job.operation, next_retry_after)
                        .await;
                });
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
        let current_realtime_timestamp = self.get_realtime_boundary();
        let new_intervals = self
            .generate_historical_intervals(current_realtime_timestamp)
            .await?;
        if new_intervals > 0 {
            tracing::info!(
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

        // Create streams
        let realtime = self.realtime_stream();
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
        let operations = self.operations_stream();
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
            realtime,
            select_with_strategy(
                operations,
                select_with_strategy(intervals_stream, retry_stream, Self::prio_left),
                Self::prio_left,
            ),
            Self::prio_left,
        );

        tracing::debug!(current_realtime_timestamp, "Starting TAC indexer");

        let job_futures = combined_stream.map(|job| async move {
            match job {
                IndexerJob::Realtime => {
                    let wm = self
                        .database
                        .get_watermark()
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to get watermark: {}", e))
                        .unwrap();
                    let now = chrono::Utc::now().timestamp() as u64;
                    if let Err(e) = self.database.generate_pending_interval(wm, now).await {
                        tracing::error!(err =? e, "Failed to generate real-time interval");
                    }
                    anyhow::Ok(())
                }
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
        });

        let mut buffered_stream = job_futures.buffer_unordered(self.settings.concurrency as usize);
        while let Some(result) = buffered_stream
            .next()
            .instrument(tracing::debug_span!(
                "job processing",
                span_id = Uuid::new_v4().to_string()
            ))
            .await
        {
            if let Err(e) = result {
                tracing::error!(err =? e, "Job processing error");
            }
        }

        Ok(())
    }
}
