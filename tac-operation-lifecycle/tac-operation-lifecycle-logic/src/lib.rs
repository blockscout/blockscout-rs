use std::{
    cmp::max, collections::HashMap, sync::{atomic::AtomicU64, Arc}, time::Duration, thread,
};

use anyhow::Error;
use client::{models::profiling::{BlockchainType, OperationType, StageType}, Client};
use database::{EntityStatus, OrderDirection, TacDatabase};
use sea_orm::{DatabaseConnection, EntityTrait};
use tac_operation_lifecycle_entity::{interval, operation, watermark};

pub mod client;
pub mod settings;
pub mod database;

use futures::{
    stream::{select_with_strategy, BoxStream, PollNext},
    StreamExt,
};
use settings::IndexerSettings;
use chrono;
use tracing::instrument;

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
    Realtime,   // to generate realtime intervals
    Interval(Job),  // to request operation IDs within interval
    Operation(OperationJob),    // to request profiling data for the operations
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

    pub fn to_string(&self) -> String { // SCREAMING_SNAKE_CASE
        serde_json::to_string(self)
            .unwrap()
            .trim_matches('"')
            .to_string()
    }

    pub fn is_finalized(&self) -> bool {
        self == &OperationType::Pending
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

    pub fn to_string(&self) -> String {
        format!("{:?}", self)
    }
}

impl BlockchainType {
    pub fn to_string(&self) -> String {
        format!("{:?}", self)
    }
}

pub struct Indexer {
    client: Client,
    settings: IndexerSettings,
    watermark: AtomicU64,
    start_timestamp: u64,
    database: Arc<TacDatabase>,
}

impl Indexer {
    pub async fn new(
        settings: IndexerSettings,
        db: Arc<DatabaseConnection>,
    ) -> anyhow::Result<Self> {
        
        let watermark = watermark::Entity::find()
            .one(db.as_ref())
            .await?
            .map_or(settings.start_timestamp as u64, |row| {
                max(row.timestamp as u64, settings.start_timestamp as u64)
            });
        Ok(Self {
            client: Client::new(settings.client.clone()),
            settings,
            watermark: AtomicU64::new(watermark),
            start_timestamp: 0,
            database: Arc::new(TacDatabase::new(db)),
        })
    }
}

impl Indexer {
    // pub async fn find_inconsistent_intervals(&self) -> anyhow::Result<()> {
    //     let intervals = interval::Entity::find()
    //         .filter(
    //             interval::Column::Start
    //                 .gt(self.watermark.load(std::sync::atomic::Ordering::Acquire) as i64),
    //         )
    //         .all(self.db.as_ref())
    //         .await?;

    //     for interval in intervals {
    //         tracing::error!("Interval {} is inconsistent", interval.id);
    //     }
    //     Ok(())
    // }

    // Generate intervals between current epoch and watermark and save them to the db
    // Returns number of the generated intervals
    #[instrument(name = "generate_historical_intervals", skip_all, level = "info")]
    pub async fn generate_historical_intervals(&self, now: u64) -> anyhow::Result<usize> {
        let watermark = self.database.get_watermark().await?;
        let catchup_period = self.settings.catchup_interval.as_secs();
        let global_start = self.settings.start_timestamp;
        
        self.database.generate_pending_intervals(max(watermark, global_start), now, catchup_period).await
    }

    pub async fn generate_realtime_intervals(&self) {
        let mut interval = tokio::time::interval(self.settings.polling_interval);
        
        loop {
            interval.tick().await;

            let wm = self.database.get_watermark().await.unwrap();
            let now = chrono::Utc::now().timestamp() as u64;
            if let Err(e) = self.database.generate_pending_interval(wm, now).await {
                tracing::error!("Failed to generate real-time interval: {}", e);
            }
        }
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

    pub fn watermark(&self) -> u64 {
        self.watermark.load(std::sync::atomic::Ordering::Acquire)
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

    pub fn interval_stream(&self, direction: OrderDirection, from: Option<u64>, to: Option<u64>) -> BoxStream<'_, IndexerJob> {
        const INTERVALS_PULL_SIZE: usize = 1;
        const INTERVALS_LOOP_DELAY_INTERVAL_MS: u64 = 100;

        Box::pin(async_stream::stream! {
            loop {
                match self.database.pull_pending_intervals(INTERVALS_PULL_SIZE, direction, from, to).await {
                    Ok(pulled) => {
                        for interval in pulled {
                            // Yield the job
                            yield IndexerJob::Interval(Job {
                                interval: interval.clone(),
                            });
                        }
                    },
                    Err(e) => {
                        tracing::error!("[Thread {:?}] Unable to pull intervals from the database: {}", thread::current().id(), e);
                    },
                }

                // Sleep a bit before next iteration to prevent tight loop
                tokio::time::sleep(Duration::from_millis(INTERVALS_LOOP_DELAY_INTERVAL_MS)).await;
            }
        })
    }

    pub fn operations_stream(&self) -> BoxStream<'_, IndexerJob> {
        const OPERATIONS_PULL_SIZE: usize = 1;
        const OPERATIONS_LOOP_DELAY_INTERVAL_MS: u64 = 200;
        Box::pin(async_stream::stream! {
            loop {
                match self.database.pull_pending_operations(OPERATIONS_PULL_SIZE, OrderDirection::EarliestFirst).await {
                    Ok(pulled) => {
                        for operation in pulled {
                            // Yield the job
                            yield IndexerJob::Operation(OperationJob {
                                operation: operation,
                            });
                        }

                        // Sleep a bit before next iteration to prevent tight loop
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    },
                    Err(e) => {
                        tracing::error!("Unable to pull operations from the database: {}", e);
                    },
                }

                // Sleep a bit before next iteration to prevent tight loop
                tokio::time::sleep(Duration::from_millis(OPERATIONS_LOOP_DELAY_INTERVAL_MS)).await;
            }
        })
    }

    pub async fn process_interval_with_retries(&self, job: &Job) -> () {
        match self.fetch_operations(&job).await {
            Ok(num) => {
                if num > 0 {
                    tracing::info!("Successfully fetched {} operations for interval {}", num, job.interval.id);
                }
            }
            
            Err(e) => {
                tracing::error!("Failed to fetch operations: {}", e);
                
                // Schedule retry with exponential backoff
                let retries = job.interval.retry_count;
                let base_delay = 5; // 5 seconds base delay
                let next_retry_after = (base_delay * 2i64.pow(retries as u32)) as i64;

                let _ = self.database.set_interval_retry(&job.interval, next_retry_after).await;
            }
        }
    }

    pub async fn fetch_operations(&self, job: &Job) -> Result<usize, Error> {
        let thread_id = thread::current().id();
        tracing::debug!("[Thread {:?}] Processing interval job: {:?}", thread_id, job);

        let operations = self.client.get_operations(job.interval.start as u64, job.interval.end as u64).await?;
        let ops_num = operations.len();
        
        if ops_num > 0 {
            tracing::info!("[Thread {:?}] Fetched {} operation_ids: [\n\t{}\n]", thread_id, ops_num, operations.iter().map(|o| o.id.clone()).collect::<Vec<_>>().join(",\n\t"));
            self.database.insert_pending_operations(&operations).await?
        }

        self.database.set_interval_status(&job.interval, EntityStatus::Finalized).await?;

        let job_type = if job.interval.start as u64 >= self.start_timestamp {
            "REALTIME"
        } else {
            "HISTORICAL"
        };
        tracing::info!("[Thread {:?}] Successfully processed {} job (id={})", 
            thread_id,
            job_type,
            job.interval.id,
        );

        Ok(ops_num)
    }

    pub async fn process_operation_with_retries(&self, jobs: Vec<&OperationJob>) -> () {
        let op_ids: Vec<&str> = jobs.iter().map(|j| j.operation.id.as_str()).collect();

        match self.client.get_operations_stages(op_ids.clone()).await {
            Ok(operations_map) => {
                let mut timestamp_acc = 0;
                let mut processed_operations = 0;
                for (op_id, operation_data) in operations_map.iter() {
                    // Find an associated operation in the input operations vector
                    match jobs.iter().find(|j| &j.operation.id == op_id) {
                        Some(job) => {
                            let _ = self.database.set_operation_data(&job.operation, operation_data).await;

                            if operation_data.operation_type != OperationType::Pending {
                                let _ = self.database.set_operation_status(&job.operation, EntityStatus::Finalized).await;
                            } else {
                                // TODO: Add operation to the fast-retry queue
                            }
                            timestamp_acc += job.operation.timestamp;
                            processed_operations = processed_operations + 1;
                        },
                        None => {
                            tracing::error!("Stage profiling response contains unknown operation (id = {}). Skipping...", op_id);
                        }
                    }
                    
                    
                }

                let avg_timestamp = timestamp_acc / processed_operations;
                if avg_timestamp > self.start_timestamp as i64 {
                    tracing::info!(
                        "Successfully processed {} REALTIME operations. Average delay = {}s: [{}]",
                        processed_operations,
                        chrono::Utc::now().timestamp() - avg_timestamp,
                        op_ids.join(",")
                    );
                } else {
                    tracing::info!(
                        "Successfully processed {} HISTORICAL operations: [{}]",
                        processed_operations,
                        op_ids.join(",")
                    );
                };
            }
            _ => {
                let _ = jobs.iter().map(|job| async {
                    let retries = job.operation.retry_count;
                    let base_delay = 5; // 5 seconds base delay
                    let next_retry_after = (base_delay * 2i64.pow(retries as u32)) as i64;

                    let _ = self.database.set_operation_retry(&job.operation, next_retry_after).await;
                });
            }
        }
    }

    fn prio_left(_: &mut ()) -> PollNext { PollNext::Left }

    #[instrument(name = "indexer", skip_all, level = "info")]
    pub async fn start(&mut self) -> anyhow::Result<()> {
        tracing::warn!("Initializing TAC indexer");

        self.ensure_stages_types_exist().await?;

        // Generate historical intervals
        self.generate_historical_intervals(chrono::Utc::now().timestamp() as u64).await?;

        // This timestamp will used for distinguish between historical and realtime intervals
        self.start_timestamp = self.database.get_watermark().await?;
        
        // Create prioritized streams
        let realtime = self.realtime_stream();
        let realtime_intervals = self.interval_stream(OrderDirection::EarliestFirst, Some(self.start_timestamp), None);
        let historical_intervals_high_priority = self.interval_stream(OrderDirection::LatestFirst, None, Some(self.start_timestamp));
        let historical_intervals_low_priority = self.interval_stream(OrderDirection::EarliestFirst, None, Some(self.start_timestamp));
        let operations = self.operations_stream();

        // Combine streams with prioritization (high priority first)
        let historical_intervals_stream = select_with_strategy(historical_intervals_high_priority, historical_intervals_low_priority, Self::prio_left);
        let intervals_stream = select_with_strategy(realtime_intervals, historical_intervals_stream, Self::prio_left);
        let mut combined_stream = select_with_strategy(
            realtime, 
            select_with_strategy(operations, intervals_stream, Self::prio_left),
            Self::prio_left
        );

        tracing::info!("Starting TAC indexer. Realtime intervals lays after timestamp: {}", self.start_timestamp);

        // Process the prioritized stream
        while let Some(job) = combined_stream.next().await {
            match job {
                IndexerJob::Realtime => {
                    let wm = self.database.get_watermark().await.unwrap();
                    let now = chrono::Utc::now().timestamp() as u64;
                    if let Err(e) = self.database.generate_pending_interval(wm, now).await {
                        tracing::error!("Failed to generate real-time interval: {}", e);
                    }
                },

                IndexerJob::Interval(job) => {
                    self.process_interval_with_retries(&job).await;
                },

                IndexerJob::Operation(job) => {
                    self.process_operation_with_retries([&job].to_vec()).await;
                }
            }
        }

        Ok(())
    }
}
