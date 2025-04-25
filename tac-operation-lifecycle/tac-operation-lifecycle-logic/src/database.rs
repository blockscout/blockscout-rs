use crate::client::models::{
    operations::Operations as ApiOperations, profiling::OperationData as ApiOperationData,
};
use anyhow::anyhow;
use sea_orm::{
    prelude::DateTime,
    sea_query::OnConflict,
    ActiveEnum, ActiveModelTrait,
    ActiveValue::{self, NotSet},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Set, Statement, TransactionTrait,
};
use std::{cmp::min, collections::HashMap, fmt, sync::Arc, time::Instant};
use tac_operation_lifecycle_entity::{
    interval,
    operation::{self, Column},
    operation_stage,
    sea_orm_active_enums::StatusEnum,
    stage_type, transaction, watermark,
};
use tracing::Instrument;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum OrderDirection {
    /// Process intervals from newest to oldest (for new jobs)
    LatestFirst,
    /// Process intervals from oldest to newest (for catch up)
    EarliestFirst,
}

impl OrderDirection {
    pub fn sql_order_string(&self) -> String {
        match self {
            OrderDirection::LatestFirst => "DESC".to_string(),
            OrderDirection::EarliestFirst => "ASC".to_owned(),
        }
    }
}

#[derive(Debug)]
pub enum EntityStatus {
    Pending,
    Processing,
    Finalized,
}

impl EntityStatus {
    pub fn to_id(&self) -> i16 {
        match self {
            EntityStatus::Pending => 0,
            EntityStatus::Processing => 1,
            EntityStatus::Finalized => 2,
        }
    }
}

impl fmt::Display for EntityStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EntityStatus::Pending => "Pending",
            EntityStatus::Processing => "Processing",
            EntityStatus::Finalized => "Finalized",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IntervalDbStatistic {
    pub first_timestamp: u64,
    pub last_timestamp: u64,
    pub total_intervals: usize,
    pub pending_intervals: usize,
    pub processing_intervals: usize,
    pub finalized_intervals: usize,
    pub failed_intervals: usize,
    pub finalized_period: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OperationDbStatistic {
    pub max_timestamp: u64,
    pub total_operations: usize,
    pub pending_operations: usize,
    pub processing_operations: usize,
    pub finalized_operations: usize,
    pub failed_operations: usize,
}

#[derive(Debug, FromQueryResult)]
struct JoinedRow {
    // operation
    op_id: String,
    op_type: Option<String>,
    timestamp: DateTime,
    status: StatusEnum,

    //operation_stage
    stage_id: Option<i32>,
    stage_type_id: i16,
    stage_success: bool,
    stage_timestamp: Option<DateTime>,
    stage_note: Option<String>,

    // transaction
    tx_id: Option<i32>,
    tx_stage_id: Option<i32>,
    tx_hash: Option<String>,
    tx_blockchain_type: Option<String>,
}

pub struct TacDatabase {
    db: Arc<DatabaseConnection>,
    start_timestamp: u64,
}

impl TacDatabase {
    pub fn new(db: Arc<DatabaseConnection>, start_timestamp: u64) -> Self {
        Self {
            db,
            start_timestamp,
        }
    }

    // Add helper function for timestamp conversion
    fn timestamp_to_naive(timestamp: i64) -> chrono::NaiveDateTime {
        chrono::DateTime::from_timestamp(timestamp, 0)
            .unwrap()
            .naive_utc()
    }

    // Retrieves the saved watermark from the database (returns 0 if not exist)
    // NOTE: the method doesn't perform any cachingm just read from the DB!
    pub async fn get_watermark(&self) -> anyhow::Result<u64> {
        let existing_watermark = watermark::Entity::find()
            .order_by_asc(watermark::Column::Timestamp)
            .all(self.db.as_ref())
            .await?;

        if existing_watermark.is_empty() {
            Ok(self.start_timestamp)
        } else {
            if existing_watermark.len() > 1 {
                tracing::error!("Found more than one watermark in the database");
            }
            Ok(existing_watermark[0].timestamp.and_utc().timestamp() as u64)
        }
    }

    // Find and update existing watermark or create new one
    pub async fn set_watermark(&self, timestamp: u64) -> anyhow::Result<()> {
        let tx = self.db.begin().await?;
        self.set_watermark_internal(&tx, timestamp).await?;
        tx.commit().await?;

        Ok(())
    }

    async fn set_watermark_internal(
        &self,
        tx: &DatabaseTransaction,
        timestamp: u64,
    ) -> anyhow::Result<()> {
        let existing_watermark = watermark::Entity::find()
            .order_by_asc(watermark::Column::Timestamp)
            .one(tx)
            .await?;

        let timestamp_naive = Self::timestamp_to_naive(timestamp as i64);

        let watermark_model = match existing_watermark {
            Some(w) => watermark::ActiveModel {
                id: ActiveValue::Unchanged(w.id),
                timestamp: ActiveValue::Set(timestamp_naive),
            },
            None => watermark::ActiveModel {
                id: ActiveValue::NotSet,
                timestamp: ActiveValue::Set(timestamp_naive),
            },
        };

        // Save the updated watermark
        watermark_model.save(tx).await?;

        Ok(())
    }

    pub async fn get_latest_operation_timestamp(&self) -> anyhow::Result<Option<u64>> {
        match operation::Entity::find()
            .select_only()
            .column_as(operation::Column::Timestamp.max(), "max_timestamp")
            .into_tuple::<Option<u64>>()
            .one(self.db.as_ref())
            .await
        {
            Ok(Some(ts)) => Ok(ts),
            Err(e) => Err(e.into()),
            _ => Ok(None),
        }
    }

    // Create interval with the specified boundary [from..to]
    // Update watermark in the database
    pub async fn generate_pending_interval(&self, from: u64, to: u64) -> anyhow::Result<()> {
        let now = chrono::Utc::now().naive_utc();

        let interval = interval::ActiveModel {
            id: ActiveValue::NotSet,
            start: ActiveValue::Set(Self::timestamp_to_naive(from as i64)),
            finish: ActiveValue::Set(Self::timestamp_to_naive(to as i64)),
            inserted_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
            status: sea_orm::ActiveValue::Set(StatusEnum::Pending),
            next_retry: ActiveValue::Set(None),
            retry_count: ActiveValue::Set(0_i16),
        };

        let tx = self.db.begin().await?;

        match interval::Entity::insert(interval).exec(&tx).await {
            Ok(_) => {
                self.set_watermark_internal(&tx, to).await?;

                tx.commit().await?;
                tracing::debug!(
                    new_watermark =? to,
                    "Pending interval was added and watermark updated to"
                );
            }

            Err(e) => {
                tx.rollback().await?;
                tracing::error!(
                    from,
                    to,
                    err =? e,
                    "Failed to add pending interval"
                );
                return Err(e.into());
            }
        };

        Ok(())
    }

    // Create intervals with the specified period within the [from..to] boundary
    // Returns the number of created intervals
    // NOTE: the routine doesn't check if intervals will overlapped with existing ones
    pub async fn generate_pending_intervals(
        &self,
        from: u64,
        to: u64,
        period_secs: u64,
    ) -> anyhow::Result<usize> {
        const DB_BATCH_SIZE: usize = 1000;
        let now = chrono::Utc::now().naive_utc();

        let intervals: Vec<interval::ActiveModel> = (from..to)
            .step_by(period_secs as usize)
            .map(|timestamp| {
                let start_naive = Self::timestamp_to_naive(timestamp as i64);
                let finish_naive =
                    Self::timestamp_to_naive(min(timestamp + period_secs, to) as i64);

                interval::ActiveModel {
                    id: ActiveValue::NotSet,
                    start: ActiveValue::Set(start_naive),
                    //we don't want to save intervals that are out of specified boundaries
                    finish: ActiveValue::Set(finish_naive),
                    inserted_at: ActiveValue::Set(now),
                    updated_at: ActiveValue::Set(now),
                    status: sea_orm::ActiveValue::Set(StatusEnum::Pending),
                    next_retry: ActiveValue::Set(None),
                    retry_count: ActiveValue::Set(0_i16),
                }
            })
            .collect();

        tracing::debug!(
            total_intervals_generated =? intervals.len(),
            interval_period_secs =? period_secs,
            from,
            to,
            "Pending intervals were generated",
        );

        // Process intervals in batches
        for chunk in intervals.chunks(DB_BATCH_SIZE) {
            let tx = self.db.begin().await?;

            match interval::Entity::insert_many(chunk.to_vec())
                .exec_with_returning(&tx)
                .await
            {
                Ok(_) => {
                    // Update watermark to the end of the last interval in this batch
                    // [assume bathes are sorted by `start` field ascending at that point]
                    let mut updated_watermark: Option<u64> = None;
                    if let Some(last_interval_from_batch) = chunk.last() {
                        if let ActiveValue::Set(finish) = &last_interval_from_batch.finish {
                            let timestamp = finish.and_utc().timestamp() as u64;
                            let _ = self.set_watermark_internal(&tx, timestamp).await;
                            updated_watermark = Some(timestamp);
                        }
                    }

                    tx.commit().await?;
                    tracing::debug!(
                        batch_size =? chunk.len(),
                        new_watermark =? if let Some(wm) = updated_watermark {
                            wm.to_string()
                        } else {
                            "[NOT_UPDATED]".to_string()
                        },
                        "Successfully saved batch of intervals and updated watermark",
                    );
                }
                Err(e) => {
                    tx.rollback().await?;
                    tracing::error!(err =? e, "Failed to save batch");
                    return Err(e.into());
                }
            }
        }

        Ok(intervals.len())
    }

    // stage_types is a vector Id->StageName
    pub async fn register_stage_types(
        &self,
        stage_types: &HashMap<i32, String>,
    ) -> anyhow::Result<()> {
        for (stage_id, stage_name) in stage_types.iter() {
            let stage_type_model = stage_type::ActiveModel {
                id: Set(*stage_id),
                name: Set(stage_name.to_string()),
            };

            stage_type::Entity::insert(stage_type_model)
                .on_conflict(
                    OnConflict::column(stage_type::Column::Id)
                        .update_columns([stage_type::Column::Name])
                        .to_owned(),
                )
                .exec(self.db.as_ref())
                .await?;
        }

        Ok(())
    }

    pub async fn insert_pending_operations(
        &self,
        operations: &ApiOperations,
    ) -> anyhow::Result<()> {
        // Start a transaction
        let txn = self.db.begin().await?;

        // Save all operations
        for op in operations {
            let now = chrono::Utc::now().naive_utc();

            let operation_model = operation::ActiveModel {
                id: Set(op.id.clone()),
                op_type: Set(None),
                timestamp: Set(Self::timestamp_to_naive(op.timestamp as i64)),
                status: Set(StatusEnum::Pending),
                next_retry: Set(None),
                retry_count: Set(0), // Initialize retry count
                inserted_at: Set(now),
                updated_at: Set(now),
            };

            tracing::debug!("Attempting to insert operation: {:?}", operation_model);

            // Use on_conflict().do_nothing() with proper error handling
            match operation::Entity::insert(operation_model)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::column(operation::Column::Id)
                        .do_nothing()
                        .to_owned(),
                )
                .exec_without_returning(&txn)
                .await
            {
                Ok(cnt) => {
                    if cnt > 0 {
                        tracing::debug!("Successfully inserted or skipped operation {}", op.id);
                    } else {
                        tracing::warn!("Operation {} skipped due to conflict", op.id);
                    }
                }
                Err(e) => {
                    tracing::error!("Error inserting operation: {:?}", e);
                    // Don't fail the entire batch for a single operation
                    continue;
                }
            }
        }

        // Commit transaction
        txn.commit().await?;

        Ok(())
    }

    pub async fn set_interval_status(
        &self,
        interval_model: &interval::Model,
        status: &StatusEnum,
    ) -> anyhow::Result<()> {
        let mut interval_active: interval::ActiveModel = interval_model.clone().into();
        interval_active.status = Set(status.clone());

        // Saving changes
        interval_active
            .update(self.db.as_ref())
            .await
            .map_err(|e| {
                tracing::error!(
                    interval_id =? interval_model.id,
                    new_status =? status.to_value(),
                    err =? e,
                    "Failed to update interval status",
                );
                anyhow!(e)
            })?;

        Ok(())
    }

    pub async fn set_interval_status_by_id(
        &self,
        interval_id: i32,
        status: &StatusEnum,
    ) -> anyhow::Result<()> {
        // Found record by PK
        match interval::Entity::find_by_id(interval_id)
            .one(self.db.as_ref())
            .await?
        {
            Some(interval_model) => self.set_interval_status(&interval_model, status).await,
            None => {
                let err = anyhow!(
                    "Cannot update interval {} status to {}: not found",
                    interval_id,
                    status.to_value()
                );
                tracing::error!("{}", err);
                Err(err)
            }
        }
    }

    pub async fn set_operation_status(
        &self,
        operation_model: &operation::Model,
        status: &StatusEnum,
    ) -> anyhow::Result<()> {
        let mut operation_active: operation::ActiveModel = operation_model.clone().into();
        operation_active.status = Set(status.clone());

        // Saving changes
        operation_active
            .update(self.db.as_ref())
            .await
            .map_err(|e| {
                tracing::error!(
                    operation_id =? operation_model.id,
                    new_status =? status.to_value(),
                    err =? e,
                    "Failed to update operation status"
                );
                anyhow!(e)
            })?;

        tracing::debug!(
            operation_id =? operation_model.id,
            new_status =? status.to_value(),
            "Successfully updated operation status"
        );
        Ok(())
    }

    pub async fn set_operation_status_by_id(
        &self,
        op_id: &String,
        status: &StatusEnum,
    ) -> anyhow::Result<()> {
        // Found record by PK
        match operation::Entity::find_by_id(op_id)
            .one(self.db.as_ref())
            .await?
        {
            Some(operation_model) => self.set_operation_status(&operation_model, status).await,
            None => {
                let err = anyhow!("operation not found");
                tracing::error!(
                    operation_id =? op_id,
                    new_status =? status.to_value(),
                    err =? err,
                    "Cannot update operation status"
                );
                Err(err)
            }
        }
    }

    // Helper function to build interval query with common structure
    fn build_interval_query(
        &self,
        conditions: Vec<String>,
        order_by: Option<(&str, OrderDirection)>,
        limit: usize,
        update_status: &StatusEnum,
    ) -> String {
        let order_clause = if let Some((field, direction)) = order_by {
            format!("ORDER BY {} {}", field, direction.sql_order_string())
        } else {
            "".to_string()
        };

        let conditions_joined = conditions.join(" AND ");
        let new_status = update_status.to_value();
        format!(
            r#"
            WITH selected_intervals AS (
                SELECT * FROM interval
                WHERE {conditions_joined}
                {order_clause}
                LIMIT {limit}
                FOR UPDATE SKIP LOCKED
            )
            UPDATE interval
            SET status = '{new_status}'::status_enum
            WHERE id IN (SELECT id FROM selected_intervals)
            RETURNING id, start, finish, status::text, next_retry, retry_count, inserted_at, updated_at
            "#
        )
    }

    // Helper function to build operation query with common structure
    fn build_operation_query(
        &self,
        conditions: Vec<String>,
        order_by: Option<(&str, OrderDirection)>,
        limit: usize,
        update_status: &StatusEnum,
    ) -> String {
        let order_clause = if let Some((field, direction)) = order_by {
            format!("ORDER BY {} {}", field, direction.sql_order_string())
        } else {
            "".to_string()
        };

        let conditions_joined = conditions.join(" AND ");
        let new_status = update_status.to_value();
        format!(
            r#"
            WITH selected_operations AS (
                SELECT * FROM operation 
                WHERE {conditions_joined} 
                {order_clause} 
                LIMIT {limit}
                FOR UPDATE SKIP LOCKED
            )
            UPDATE operation 
            SET status = '{new_status}'::status_enum
            WHERE id IN (SELECT id FROM selected_operations)
            RETURNING id, timestamp, status::text, next_retry, retry_count, inserted_at, updated_at
            "#,
        )
    }

    fn format_sql_for_logging(sql: &str) -> String {
        sql.replace("\n", " ").replace("  ", " ")
    }

    pub async fn query_pending_intervals(
        &self,
        num: usize,
        order: OrderDirection,
        from: Option<u64>,
        to: Option<u64>,
    ) -> anyhow::Result<Vec<interval::Model>> {
        let mut conditions = vec![format!(
            "status = '{}'::status_enum",
            StatusEnum::Pending.to_value()
        )];
        if let Some(start) = from {
            let start_naive = Self::timestamp_to_naive(start as i64);
            conditions.push(format!("start >= '{}'", start_naive));
        }
        if let Some(finish) = to {
            let finish_naive = Self::timestamp_to_naive(finish as i64);
            conditions.push(format!(r#"finish < '{}'"#, finish_naive));
        }

        let sql = self.build_interval_query(
            conditions,
            Some(("start", order)),
            num,
            &StatusEnum::Processing,
        );

        self.query_intervals(&sql)
            .instrument(tracing::debug_span!(
                "query pending intervals",
                sql = Self::format_sql_for_logging(&sql)
            ))
            .await
    }

    pub async fn query_failed_intervals(&self, num: usize) -> anyhow::Result<Vec<interval::Model>> {
        let conditions = vec![
            format!("status = '{}'::status_enum", StatusEnum::Pending.to_value()),
            format!("next_retry IS NOT NULL"),
            format!(
                "next_retry < '{}'",
                Self::timestamp_to_naive(chrono::Utc::now().timestamp())
            ),
        ];

        let sql = self.build_interval_query(
            conditions,
            Some(("next_retry", OrderDirection::EarliestFirst)),
            num,
            &StatusEnum::Processing,
        );

        let span_id = Uuid::new_v4();
        self.query_intervals(&sql)
            .instrument(tracing::debug_span!(
                "query failed intervals",
                span_id = span_id.to_string()
            ))
            .await
    }

    async fn query_intervals(&self, sql: &String) -> anyhow::Result<Vec<interval::Model>> {
        // Generate a unique transaction ID for tracking
        let tx_id = Uuid::new_v4();

        // Create the statement
        let update_stmt =
            Statement::from_sql_and_values(sea_orm::DatabaseBackend::Postgres, sql, vec![]);

        // Execute the UPDATE with RETURNING clause directly on the database connection
        match interval::Entity::find()
            .from_raw_sql(update_stmt)
            .all(self.db.as_ref())
            .instrument(tracing::debug_span!(
                "executing update query for intervals",
                tx_id = tx_id.to_string(),
                sql = sql.as_str()
            ))
            .await
        {
            Ok(intervals) => {
                tracing::debug!(
                    intervals_count =? intervals.len(),
                    "Intervals were updated"
                );
                Ok(intervals)
            }
            Err(e) => {
                tracing::error!(err =? e, "Failed to update intervals");
                Err(e.into())
            }
        }
    }

    // Extract up to `num` operations in the pending state and switch them status to `processing`
    pub async fn query_pending_operations(
        &self,
        num: usize,
        order: OrderDirection,
    ) -> anyhow::Result<Vec<operation::Model>> {
        let conditions = vec![format!(
            "status = '{}'::status_enum",
            StatusEnum::Pending.to_value()
        )];

        let sql = self.build_operation_query(
            conditions,
            Some(("timestamp", order)),
            num,
            &StatusEnum::Processing,
        );

        self.query_operations(&sql)
            .instrument(tracing::debug_span!("query pending operations"))
            .await
    }

    pub async fn query_failed_operations(
        &self,
        num: usize,
        order: OrderDirection,
    ) -> anyhow::Result<Vec<operation::Model>> {
        let conditions = vec![
            format!("status = '{}'::status_enum", StatusEnum::Pending.to_value()),
            format!("next_retry IS NOT NULL"),
            format!(
                "next_retry < '{}'",
                Self::timestamp_to_naive(chrono::Utc::now().timestamp())
            ),
        ];

        let sql = self.build_operation_query(
            conditions,
            Some(("next_retry", order)),
            num,
            &StatusEnum::Processing,
        );

        let span_id = Uuid::new_v4();
        self.query_operations(&sql)
            .instrument(tracing::debug_span!(
                "query failed operations",
                span_id = span_id.to_string(),
                sql = Self::format_sql_for_logging(sql.as_str())
            ))
            .await
    }

    async fn query_operations(&self, sql: &String) -> anyhow::Result<Vec<operation::Model>> {
        // Generate a unique transaction ID for tracking
        let tx_id = Uuid::new_v4();

        // Create the statement
        let update_stmt =
            Statement::from_sql_and_values(sea_orm::DatabaseBackend::Postgres, sql, vec![]);

        // Execute the UPDATE with RETURNING clause directly on the database connection
        match operation::Entity::find()
            .from_raw_sql(update_stmt)
            .all(self.db.as_ref())
            .instrument(tracing::debug_span!(
                "executing update query for operations",
                tx_id = tx_id.to_string(),
                sql = Self::format_sql_for_logging(sql.as_str())
            ))
            .await
        {
            Ok(operations) => {
                tracing::debug!(
                    operations_count =? operations.len(),
                    "Operations were updated"
                );
                Ok(operations)
            }
            Err(e) => {
                tracing::error!(err =? e, "Failed to update operations");
                Err(e.into())
            }
        }
    }

    pub async fn set_operation_data(
        &self,
        operation: &operation::Model,
        operation_data: &ApiOperationData,
    ) -> anyhow::Result<()> {
        let tx_id = Uuid::new_v4();
        let start_time = Instant::now();
        tracing::debug!(tx_id =? tx_id, "Beginning transaction for set_operation_data");

        let txn = match self.db.begin().await {
            Ok(txn) => txn,
            Err(e) => {
                tracing::error!(
                    tx_id =? tx_id,
                    time_elapsed_ms =? start_time.elapsed().as_millis(),
                    err =? e,
                    "Failed to begin transaction",
                );
                return Err(anyhow!(e));
            }
        };

        // Remove associated stages with transactions
        if let Err(e) = operation_stage::Entity::delete_many()
            .filter(operation_stage::Column::OperationId.eq(&operation.id))
            .exec(&txn)
            .await
        {
            tracing::error!(
                operation_id =? operation.id,
                err =? e,
                "Failed to delete existing stages for operation"
            );
            let _ = txn.rollback().await;
            return Err(e.into());
        }

        // Update operation type and status
        let mut operation_model: operation::ActiveModel = operation.clone().into();
        if operation_data.operation_type.is_finalized() {
            operation_model.status = Set(StatusEnum::Completed);
        }
        operation_model.op_type = Set(Some(operation_data.operation_type.to_string()));

        if let Err(e) = operation_model
            .update(&txn)
            .instrument(tracing::debug_span!(
                "updating operation",
                tx_id = tx_id.to_string()
            ))
            .await
        {
            tracing::error!(
                operation_id =? operation.id,
                err =? e, "Failed to update operation status"
            );
            let _ = txn.rollback().await;

            return Err(e.into());
        }

        // Store operation stages
        for (stage_type, stage_data) in operation_data.stages.iter() {
            if let Some(data) = &stage_data.stage_data {
                let now = chrono::Utc::now().naive_utc();

                let stage_model = operation_stage::ActiveModel {
                    id: NotSet,
                    operation_id: Set(operation.id.clone()),
                    stage_type_id: Set(stage_type.to_id() as i16),
                    success: Set(data.success),
                    timestamp: Set(Self::timestamp_to_naive(data.timestamp as i64)),
                    note: Set(data.note.clone()),
                    inserted_at: Set(now),
                };

                match operation_stage::Entity::insert(stage_model)
                    .on_conflict(
                        sea_orm::sea_query::OnConflict::column(operation::Column::Id)
                            .do_nothing()
                            .to_owned(),
                    )
                    .exec_with_returning(&txn)
                    .await
                {
                    Ok(inserted_stage) => {
                        tracing::debug!(
                            stage_type =? inserted_stage.stage_type_id,
                            operation_id =? operation.id,
                            "Successfully inserted stage for operation"
                        );

                        // store transactions for this stage
                        for tx in data.transactions.iter() {
                            let now = chrono::Utc::now().naive_utc();

                            let tx_model = transaction::ActiveModel {
                                id: NotSet,
                                stage_id: Set(inserted_stage.id),
                                hash: Set(tx.hash.clone()),
                                blockchain_type: Set(tx.blockchain_type.to_string()),
                                inserted_at: Set(now),
                            };

                            match transaction::Entity::insert(tx_model).exec(&txn).await {
                                Ok(_) => tracing::debug!(
                                    stage_id =? inserted_stage.id,
                                    "Successfully inserted transaction for stage",
                                ),
                                Err(e) => tracing::error!(
                                    stage_id =? inserted_stage.id,
                                    err =? e,
                                    "Error inserting transaction for stage"
                                ),
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!(err =? e, "Error inserting stage");
                        // Don't fail the entire batch for a single operation
                        continue;
                    }
                }
            }
        }

        // Commit transaction
        let commit_start = Instant::now();

        match txn
            .commit()
            .instrument(tracing::debug_span!(
                "commiting insert transaction",
                tx_id = tx_id.to_string()
            ))
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(
                    tx_id =? tx_id,
                    time_elapsed_ms =? commit_start.elapsed().as_millis(),
                    err =? e,
                    "Failed to commit transaction",
                );
                Err(e.into())
            }
        }
    }

    pub async fn set_interval_retry(
        &self,
        interval: &interval::Model,
        retry_after_delay_sec: i64,
    ) -> anyhow::Result<()> {
        // Update interval with next retry timestamp and increment retry count
        let mut interval_model: interval::ActiveModel = interval.clone().into();
        let now = chrono::Utc::now().naive_utc();

        interval_model.next_retry = Set(Some(
            chrono::Utc::now().naive_utc() + chrono::Duration::seconds(retry_after_delay_sec),
        ));
        interval_model.retry_count = Set(interval.retry_count + 1);
        interval_model.status = Set(StatusEnum::Pending); // Reset status to pending
        interval_model.updated_at = Set(now);

        match interval_model.update(self.db.as_ref()).await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(
                    interval_id =? interval.id,
                    err =? e,
                    "Failed to update interval for retry"
                );
                Err(e.into())
            }
        }
    }

    pub async fn set_operation_retry(
        &self,
        operation: &operation::Model,
        retry_after_delay_sec: i64,
    ) -> anyhow::Result<()> {
        // Update operation with next retry timestamp and increment retry count
        let mut operation_model: operation::ActiveModel = operation.clone().into();
        let now = chrono::Utc::now().naive_utc();

        operation_model.next_retry = Set(Some(
            chrono::Utc::now().naive_utc() + chrono::Duration::seconds(retry_after_delay_sec),
        ));
        operation_model.retry_count = Set(operation.retry_count + 1);
        operation_model.status = Set(StatusEnum::Pending); // Reset status to pending
        operation_model.updated_at = Set(now);

        match operation_model.update(self.db.as_ref()).await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(
                    interval_id =? operation.id,
                    err =? e,
                    "Failed to update operation for retry"
                );
                Err(e.into())
            }
        }
    }

    pub async fn get_intervals_statistic(&self) -> anyhow::Result<IntervalDbStatistic> {
        let sql = Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!(
                r#"
                WITH interval_stats AS (
                    SELECT
                        MIN(start) AS min_start,
                        MAX(finish) AS max_finish,
                        COUNT(*) AS total_intervals,
                        COUNT(CASE WHEN status = '{}'::status_enum THEN 1 END) AS pending_intervals,
                        COUNT(CASE WHEN status = '{}'::status_enum THEN 1 END) AS processing_intervals,
                        COUNT(CASE WHEN status = '{}'::status_enum THEN 1 END) AS finalized_intervals,
                        COUNT(CASE WHEN status != '{}'::status_enum AND retry_count > 0 THEN 1 END) AS failed_intervals,
                        SUM(CASE WHEN status = '{}'::status_enum 
                                THEN EXTRACT(EPOCH FROM finish - start) ELSE 0 END
                            )::BIGINT AS finalized_period
                    FROM interval
                )
                SELECT * FROM interval_stats
                "#,
                StatusEnum::Pending.to_value(),
                StatusEnum::Processing.to_value(),
                StatusEnum::Completed.to_value(),
                StatusEnum::Completed.to_value(),
                StatusEnum::Completed.to_value(),
            ),
        );

        match self.db.query_one(sql).await? {
            Some(interval_data) => {
                let first_timestamp = interval_data
                    .try_get::<Option<DateTime>>("", "min_start")?
                    .map(|dt| dt.and_utc().timestamp() as u64)
                    .unwrap_or(0);
                let last_timestamp = interval_data
                    .try_get::<Option<DateTime>>("", "max_finish")?
                    .map(|dt| dt.and_utc().timestamp() as u64)
                    .unwrap_or(0);
                let total_intervals = interval_data.try_get::<i64>("", "total_intervals")? as usize;
                let pending_intervals =
                    interval_data.try_get::<i64>("", "pending_intervals")? as usize;
                let processing_intervals =
                    interval_data.try_get::<i64>("", "processing_intervals")? as usize;
                let finalized_intervals =
                    interval_data.try_get::<i64>("", "finalized_intervals")? as usize;
                let failed_intervals =
                    interval_data.try_get::<i64>("", "failed_intervals")? as usize;
                let finalized_period = interval_data.try_get::<i64>("", "finalized_period")? as u64;

                Ok(IntervalDbStatistic {
                    first_timestamp,
                    last_timestamp,
                    total_intervals,
                    failed_intervals,
                    pending_intervals,
                    processing_intervals,
                    finalized_intervals,
                    finalized_period,
                })
            }

            _ => Ok(IntervalDbStatistic::default()),
        }
    }

    pub async fn get_operations_statistic(&self) -> anyhow::Result<OperationDbStatistic> {
        let sql = Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!(
                r#"
                WITH operation_stats AS (
                    SELECT
                        MAX(timestamp) AS max_timestamp,
                        COUNT(*) AS total_operations,
                        COUNT(CASE WHEN status = '{}'::status_enum THEN 1 END) AS pending_operations,
                        COUNT(CASE WHEN status = '{}'::status_enum THEN 1 END) AS processing_operations,
                        COUNT(CASE WHEN status = '{}'::status_enum THEN 1 END) AS finalized_operations,
                        COUNT(CASE WHEN status != '{}'::status_enum AND retry_count > 0 THEN 1 END) AS failed_operations
                    FROM operation
                )
                SELECT * FROM operation_stats
                "#,
                StatusEnum::Pending.to_value(),
                StatusEnum::Processing.to_value(),
                StatusEnum::Completed.to_value(),
                StatusEnum::Completed.to_value(),
            ),
        );

        match self.db.query_one(sql).await? {
            Some(operation_data) => {
                let max_timestamp = operation_data
                    .try_get::<Option<DateTime>>("", "max_timestamp")?
                    .map(|dt| dt.and_utc().timestamp() as u64)
                    .unwrap_or(0);
                let total_operations =
                    operation_data.try_get::<i64>("", "total_operations")? as usize;
                let pending_operations =
                    operation_data.try_get::<i64>("", "pending_operations")? as usize;
                let processing_operations =
                    operation_data.try_get::<i64>("", "processing_operations")? as usize;
                let finalized_operations =
                    operation_data.try_get::<i64>("", "finalized_operations")? as usize;
                let failed_operations =
                    operation_data.try_get::<i64>("", "failed_operations")? as usize;

                Ok(OperationDbStatistic {
                    max_timestamp,
                    total_operations,
                    pending_operations,
                    processing_operations,
                    finalized_operations,
                    failed_operations,
                })
            }

            _ => Ok(OperationDbStatistic::default()),
        }
    }

    pub async fn get_operation_by_id(
        &self,
        id: &String,
    ) -> anyhow::Result<
        Option<(
            operation::Model,
            Vec<(operation_stage::Model, Vec<transaction::Model>)>,
        )>,
    > {
        let sql = r#"
            SELECT 
                o.id as op_id, o.op_type, o.timestamp, o.status::text,
                s.id as stage_id, s.stage_type_id, s.success as stage_success, s.timestamp as stage_timestamp, s.note as stage_note,
                t.id as tx_id, t.stage_id as tx_stage_id, t.hash as tx_hash, t.blockchain_type as tx_blockchain_type
            FROM operation o
            LEFT JOIN operation_stage s ON o.id = s.operation_id
            LEFT JOIN transaction t ON s.id = t.stage_id
            WHERE o.id = $1
            "#;

        self.get_full_operation_with_sql(&sql.into(), [id.into()])
            .await
    }

    pub async fn get_operation_by_tx_hash(
        &self,
        tx_hash: &String,
    ) -> anyhow::Result<
        Option<(
            operation::Model,
            Vec<(operation_stage::Model, Vec<transaction::Model>)>,
        )>,
    > {
        let sql = r#"
            SELECT 
                o.id as op_id, o.op_type, o.timestamp, o.status::text,
                s.id as stage_id, s.stage_type_id, s.success as stage_success, s.timestamp as stage_timestamp, s.note as stage_note,
                t.id as tx_id, t.stage_id as tx_stage_id, t.hash as tx_hash, t.blockchain_type as tx_blockchain_type
            FROM operation o
            LEFT JOIN operation_stage s ON o.id = s.operation_id
            LEFT JOIN transaction t ON s.id = t.stage_id
            WHERE o.id = (
                SELECT o2.id
                FROM operation o2
                JOIN operation_stage s2 ON o2.id = s2.operation_id
                JOIN transaction t2 ON s2.id = t2.stage_id
                WHERE t2.hash = $1
                LIMIT 1
            )
            "#;

        self.get_full_operation_with_sql(&sql.into(), [tx_hash.into()])
            .await
    }

    async fn get_full_operation_with_sql(
        &self,
        sql: &String,
        values: impl IntoIterator<Item = sea_orm::Value>,
    ) -> anyhow::Result<
        Option<(
            operation::Model,
            Vec<(operation_stage::Model, Vec<transaction::Model>)>,
        )>,
    > {
        let joined: Vec<JoinedRow> =
            JoinedRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Postgres,
                sql,
                values,
            ))
            .all(self.db.as_ref())
            .await?;

        if joined.is_empty() {
            return Ok(None);
        }

        let op_row = &joined[0];
        let now = chrono::Utc::now().naive_utc();

        let op_model = operation::Model {
            id: op_row.op_id.clone(),
            op_type: op_row.op_type.clone(),
            timestamp: op_row.timestamp,
            next_retry: None,
            status: op_row.status.clone(),
            retry_count: 0,
            inserted_at: now,
            updated_at: now,
        };

        use std::collections::HashMap;

        let mut stages_map: HashMap<i32, (operation_stage::Model, Vec<transaction::Model>)> =
            HashMap::new();

        for row in joined {
            if let Some(stage_id) = row.stage_id {
                let entry = stages_map.entry(stage_id).or_insert_with(|| {
                    (
                        operation_stage::Model {
                            id: stage_id,
                            operation_id: op_model.id.clone(),
                            stage_type_id: row.stage_type_id,
                            success: row.stage_success,
                            timestamp: row.stage_timestamp.unwrap_or_default(),
                            note: row.stage_note.clone(),
                            inserted_at: now,
                        },
                        vec![],
                    )
                });

                if let Some(tx_id) = row.tx_id {
                    let tx = transaction::Model {
                        id: tx_id,
                        stage_id: row.tx_stage_id.unwrap_or_default(),
                        hash: row.tx_hash.clone().unwrap_or_default(),
                        blockchain_type: row.tx_blockchain_type.clone().unwrap_or_default(),
                        inserted_at: now,
                    };
                    entry.1.push(tx);
                }
            }
        }

        let mut stages: Vec<_> = stages_map.into_values().collect();

        stages.sort_by_key(|(stage, _)| stage.timestamp);

        Ok(Some((op_model, stages)))
    }

    pub async fn get_operations(
        &self,
        count: usize,
        earlier_timestamp: Option<u64>,
        sort: OrderDirection,
    ) -> anyhow::Result<Vec<operation::Model>> {
        let mut query = operation::Entity::find();

        if let Some(ts) = earlier_timestamp {
            query = query.filter(Column::Timestamp.lt(Self::timestamp_to_naive(ts as i64)));
        }

        query = match sort {
            OrderDirection::EarliestFirst => query.order_by_asc(Column::Timestamp),
            OrderDirection::LatestFirst => query.order_by_desc(Column::Timestamp),
        };

        let operations = query.limit(count as u64).all(self.db.as_ref()).await?;

        Ok(operations)
    }

    pub async fn reset_processing_intervals(&self) -> anyhow::Result<usize> {
        let result = interval::Entity::update_many()
            .col_expr(interval::Column::Status, StatusEnum::Pending.as_enum())
            .filter(interval::Column::Status.eq(StatusEnum::Processing))
            .exec(self.db.as_ref())
            .await?;

        Ok(result.rows_affected as usize)
    }

    pub async fn reset_processing_operations(&self) -> anyhow::Result<usize> {
        let result = operation::Entity::update_many()
            .col_expr(operation::Column::Status, StatusEnum::Pending.as_enum())
            .filter(operation::Column::Status.eq(StatusEnum::Processing))
            .exec(self.db.as_ref())
            .await?;

        Ok(result.rows_affected as usize)
    }
}
