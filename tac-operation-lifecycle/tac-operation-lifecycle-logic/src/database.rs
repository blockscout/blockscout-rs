use crate::client::models::operations::Operations as ApiOperations;
use crate::client::models::profiling::OperationData as ApiOperationData;
use anyhow::anyhow;
use sea_orm::{
    sea_query::{ArrayType, Expr, Func, OnConflict},
    ActiveModelTrait,
    ActiveValue::{self, NotSet},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, QuerySelect, Set, Statement, TransactionTrait, Value,
};
use std::{cmp::min, collections::HashMap, sync::Arc, thread, time::Instant};
use tac_operation_lifecycle_entity::{
    interval, operation, operation_stage, stage_type, transaction, watermark,
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

    pub fn to_string(&self) -> String {
        format!("{:?}", self)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DatabaseStatistic {
    pub watermark: u64,
    pub first_timestamp: u64,
    pub last_timestamp: u64,
    pub total_intervals: usize,
    pub failed_intervals: usize,
    pub total_pending_intervals: usize,
    pub historical_pending_intervals: usize,
    pub realtime_pending_intervals: usize,
    pub historical_processed_period: u64,
    pub realtime_processed_period: u64,
}

pub struct TacDatabase {
    db: Arc<DatabaseConnection>,
}

impl TacDatabase {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    // Retrieves the saved watermark from the database (returns 0 if not exist)
    // NOTE: the method doesn't perform any cachingm just read from the DB!
    pub async fn get_watermark(&self) -> anyhow::Result<u64> {
        let existing_watermark = watermark::Entity::find().one(self.db.as_ref()).await?;

        match existing_watermark {
            Some(w) => Ok(w.timestamp as u64),
            _ => Ok(0),
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
        let existing_watermark = watermark::Entity::find().one(tx).await?;

        let watermark_model = match existing_watermark {
            Some(w) => watermark::ActiveModel {
                id: ActiveValue::Unchanged(w.id),
                timestamp: ActiveValue::Set(timestamp as i64),
            },
            None => watermark::ActiveModel {
                id: ActiveValue::NotSet,
                timestamp: ActiveValue::Set(timestamp as i64),
            },
        };

        // Save the updated watermark
        watermark_model.save(tx).await?;

        Ok(())
    }

    // Create interval with the specified boundary [from..to]
    // Update watermark in the database
    pub async fn generate_pending_interval(&self, from: u64, to: u64) -> anyhow::Result<()> {
        let interval = interval::ActiveModel {
            id: ActiveValue::NotSet,
            start: ActiveValue::Set(from as i64),
            end: ActiveValue::Set(to as i64),
            timestamp: ActiveValue::Set(chrono::Utc::now().timestamp()),
            status: sea_orm::ActiveValue::Set(EntityStatus::Pending.to_id()),
            next_retry: ActiveValue::Set(None),
            retry_count: ActiveValue::Set(0 as i16),
        };

        let tx = self.db.begin().await?;

        match interval::Entity::insert(interval).exec(&tx).await {
            Ok(_) => {
                let _ = self.set_watermark_internal(&tx, to).await?;

                tx.commit().await?;
                tracing::info!("Pending interval was added and watermark updated to {}", to);
            }

            Err(e) => {
                tx.rollback().await?;
                tracing::error!("Failed to add pending interval [{}..{}]: {}", from, to, e);
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
        let intervals: Vec<interval::ActiveModel> = (from..to)
            .step_by(period_secs as usize)
            .map(|timestamp| interval::ActiveModel {
                id: ActiveValue::NotSet,
                start: ActiveValue::Set(timestamp as i64),
                //we don't want to save intervals that are out of specifiedboundaries
                end: ActiveValue::Set(min(timestamp + period_secs, to) as i64),
                timestamp: ActiveValue::Set(chrono::Utc::now().timestamp()),
                status: sea_orm::ActiveValue::Set(EntityStatus::Pending.to_id()),
                next_retry: ActiveValue::Set(None),
                retry_count: ActiveValue::Set(0 as i16),
            })
            .collect();

        tracing::info!(
            "Total {}s intervals were generated: {} ({}..{})",
            period_secs,
            intervals.len(),
            from,
            to
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
                        updated_watermark =
                            Some(last_interval_from_batch.end.clone().unwrap() as u64);
                        let _ = self
                            .set_watermark_internal(&tx, updated_watermark.unwrap())
                            .await;
                    }

                    tx.commit().await?;
                    tracing::info!(
                        "Successfully saved batch of {} intervals and updated watermark to {}",
                        chunk.len(),
                        if let Some(wm) = updated_watermark {
                            wm.to_string()
                        } else {
                            "[NOT_UPDATED]".to_string()
                        },
                    );
                }
                Err(e) => {
                    tx.rollback().await?;
                    tracing::error!("Failed to save batch: {}", e);
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
        for (stage_id, stage_name) in stage_types.into_iter() {
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
        let thread_id = thread::current().id();

        // Start a transaction
        let txn = self.db.begin().await?;

        // Save all operations
        for op in operations {
            let operation_model = operation::ActiveModel {
                id: Set(op.id.clone()),
                operation_type: Set(None),
                timestamp: Set(op.timestamp as i64),
                status: Set(EntityStatus::Pending.to_id() as i32),
                next_retry: Set(None),
                retry_count: Set(0), // Initialize retry count
            };

            tracing::debug!(
                "[Thread {:?}] Attempting to insert operation: {:?}",
                thread_id,
                operation_model
            );

            // Use on_conflict().do_nothing() with proper error handling
            match operation::Entity::insert(operation_model)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::column(operation::Column::Id)
                        .do_nothing()
                        .to_owned(),
                )
                .exec(&txn)
                .await
            {
                Ok(_) => tracing::debug!(
                    "[Thread {:?}] Successfully inserted or skipped operation",
                    thread_id
                ),
                Err(e) => {
                    tracing::debug!(
                        "[Thread {:?}] Error inserting operation: {:?}",
                        thread_id,
                        e
                    );
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
        status: EntityStatus,
    ) -> anyhow::Result<()> {
        let mut interval_active: interval::ActiveModel = interval_model.clone().into();
        interval_active.status = Set(status.to_id());

        // Saving changes
        interval_active
            .update(self.db.as_ref())
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to update interval {} status to {}: {:?}",
                    interval_model.id,
                    status.to_string(),
                    e
                );
                anyhow!(e)
            })?;

        //tracing::info!("Successfully updated interval {} status to {}", interval_id, status.to_string());
        Ok(())
    }

    pub async fn set_interval_status_by_id(
        &self,
        interval_id: i32,
        status: EntityStatus,
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
                    status.to_string()
                );
                tracing::error!("{}", err);
                Err(err)
            }
        }
    }

    pub async fn set_operation_status(
        &self,
        operation_model: &operation::Model,
        status: EntityStatus,
    ) -> anyhow::Result<()> {
        let mut operation_active: operation::ActiveModel = operation_model.clone().into();
        operation_active.status = Set(status.to_id() as i32);

        // Saving changes
        operation_active
            .update(self.db.as_ref())
            .await
            .map_err(|e| {
                tracing::error!(
                    "Failed to update operation {} status to {}: {:?}",
                    operation_model.id,
                    status.to_string(),
                    e
                );
                anyhow!(e)
            })?;

        tracing::info!(
            "Successfully updated operation {} status to {}",
            operation_model.id,
            status.to_string()
        );
        Ok(())
    }

    pub async fn set_operation_status_by_id(
        &self,
        op_id: &String,
        status: EntityStatus,
    ) -> anyhow::Result<()> {
        // Found record by PK
        match operation::Entity::find_by_id(op_id)
            .one(self.db.as_ref())
            .await?
        {
            Some(operation_model) => self.set_operation_status(&operation_model, status).await,
            None => {
                let err = anyhow!(
                    "Cannot update operation {} status to {}: not found",
                    op_id,
                    status.to_string()
                );
                tracing::error!("{}", err);
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
        update_status: EntityStatus,
    ) -> String {
        let order_clause = if let Some((field, direction)) = order_by {
            format!("ORDER BY {} {}", field, direction.sql_order_string())
        } else {
            "".to_string()
        };

        format!(
            r#" WITH selected_intervals AS (
                    SELECT * FROM interval 
                    WHERE {} 
                    {} 
                    LIMIT {}
                    FOR UPDATE SKIP LOCKED
                    )
                    UPDATE interval 
                    SET status = {}
                    WHERE id IN (SELECT id FROM selected_intervals)
                    "#,
            conditions.join(" AND "),
            order_clause,
            limit,
            update_status.to_id(),
        )
    }

    pub async fn query_pending_intervals(
        &self,
        num: usize,
        order: OrderDirection,
        from: Option<u64>,
        to: Option<u64>,
    ) -> anyhow::Result<Vec<interval::Model>> {
        let mut conditions = vec![format!("status = {}", EntityStatus::Pending.to_id())];
        if let Some(start) = from {
            conditions.push(format!("start >= {}", start));
        }
        if let Some(end) = to {
            conditions.push(format!(r#""end" < {}"#, end));
        }
        
        let sql = self.build_interval_query(
            conditions,
            Some(("start", order)),
            num,
            EntityStatus::Processing,
        );
        
        self.query_intervals(&sql)
        .instrument(tracing::info_span!(
            "query pending intervals"
        ))
        .await
    }

    pub async fn select_failed_intervals(
        &self,
        num: usize,
    ) -> anyhow::Result<Vec<interval::Model>> {
        let conditions = vec![
            format!("status = {}", EntityStatus::Pending.to_id()),
            format!("next_retry IS NOT NULL"),
            format!("next_retry < {}", chrono::Utc::now().timestamp()),
        ];
        
        let sql = self.build_interval_query(
            conditions,
            Some(("next_retry", OrderDirection::EarliestFirst)),
            num,
            EntityStatus::Processing,
        );
        
        let span_id = Uuid::new_v4();
        self.query_intervals(&sql).instrument(tracing::info_span!(
            "query failed intervals",
            span_id = span_id.to_string()
        ))
        .await
    }

    async fn query_intervals(&self, sql: &String) -> anyhow::Result<Vec<interval::Model>> {
        // Generate a unique transaction ID for tracking
        let start_time = Instant::now();
        let tx_id = Uuid::new_v4();
        let tx_start = Instant::now();

        // Start a transaction
        let txn = match self.db.begin()
        .instrument(tracing::info_span!(
            "begin transaction for update intervals",
            tx_id = tx_id.to_string(),
            sql = sql.clone()
        ))
        .await {
            Ok(txn) => {
                tracing::warn!(
                    "[TX-{:?}] INTERVALS transaction started after {:?}ms",
                    tx_id,
                    tx_start.elapsed().as_millis()
                );
                txn
            }
            Err(e) => {
                tracing::error!(
                    "[TX-{:?}] Failed to begin INTERVALS transaction: {}",
                    tx_id,
                    e
                );
                return Err(anyhow!(e));
            }
        };

        let query_start = Instant::now();
        let update_stmt = Statement::from_sql_and_values(sea_orm::DatabaseBackend::Postgres, sql, vec![]);

        // Execute the UPDATE with RETURNING clause
        let updated_intervals = match interval::Entity::find()
            .from_raw_sql(update_stmt)
            .all(&txn)
            .instrument(tracing::info_span!(
                "executing update query for intervals",
                tx_id = tx_id.to_string()
            ))
            .await
        {
            Ok(intervals) => {
                tracing::debug!(
                    "[TX-{:?}] UPDATE query completed in {:?}ms, updated {} intervals",
                    tx_id,
                    query_start.elapsed().as_millis(),
                    intervals.len()
                );
                intervals
            }
            Err(e) => {
                tracing::error!(
                    "[TX-{:?}] Failed to update intervals: {}",
                    tx_id,
                    e
                );
                let _ = txn.rollback().await;
                return Err(e.into());
            }
        };

        // Commit the transaction
        let commit_start = Instant::now();
        if let Err(e) = txn
            .commit()
            .instrument(tracing::info_span!(
                "commit update intervals",
                tx_id = tx_id.to_string()
            ))
            .await
        {
            tracing::error!(
                "[TX-{:?}] Failed to commit transaction: {}",
                tx_id,
                e
            );
            return Err(e.into());
        }

        tracing::info!(
            "[TX-{:?}] Transaction committed in {:?}ms",
            tx_id,
            commit_start.elapsed().as_millis()
        );
        tracing::info!(
            "[TX-{:?}] Complete query_intervals operation took {:?}ms",
            tx_id,
            start_time.elapsed().as_millis()
        );

        Ok(updated_intervals)
    }

    // Extract up to `num` operations in the pending state and switch them status to `processing`
    pub async fn query_pending_operations(
        &self,
        num: usize,
        order: OrderDirection,
    ) -> anyhow::Result<Vec<operation::Model>> {
        let sql = format!(
            r#" SELECT *
                                      FROM operation
                                      WHERE status = {}
                                      ORDER BY timestamp {}
                                      LIMIT {}
                                      FOR UPDATE SKIP LOCKED"#,
            EntityStatus::Pending.to_id(),
            order.sql_order_string(),
            num
        );

        self.query_operations(&sql).await
    }

    pub async fn query_failed_operations(
        &self,
        num: usize,
        order: OrderDirection,
    ) -> anyhow::Result<Vec<operation::Model>> {
        let sql = format!(
            r#" SELECT * 
                                      FROM operation 
                                      WHERE status = {} AND next_retry IS NOT NULL AND next_retry < {}
                                      ORDER BY timestamp {} 
                                      LIMIT {} 
                                      FOR UPDATE SKIP LOCKED"#,
            EntityStatus::Pending.to_id(),
            chrono::Utc::now().timestamp(),
            order.sql_order_string(),
            num
        );

        self.query_operations(&sql).await
    }

    async fn query_operations(&self, sql: &String) -> anyhow::Result<Vec<operation::Model>> {
        // Generate a unique transaction ID for tracking
        let tx_id = Uuid::new_v4();
        let start_time = Instant::now();
        tracing::warn!("[TX-{:?}] Beginning query_operations operation", tx_id);

        // Use two separate transactions for better performance
        // First transaction: SELECT the operations with FOR UPDATE SKIP LOCKED
        let select_tx_id = Uuid::new_v4();
        let select_start = Instant::now();
        tracing::debug!("[TX-{:?}] Beginning SELECT transaction", select_tx_id);

        // Find and lock pending operations
        let stmt = Statement::from_sql_and_values(sea_orm::DatabaseBackend::Postgres, sql, vec![]);

        // Start transaction
        let txn = match self.db.begin()
        .instrument(tracing::info_span!(
            "begin transaction for select operations",
            tx_id = select_tx_id.to_string()
        ))
        .await {
            Ok(txn) => {
                tracing::debug!(
                    "[TX-{:?}] SELECT transaction started after {:?}ms",
                    select_tx_id,
                    select_start.elapsed().as_millis()
                );
                txn
            }
            Err(e) => {
                tracing::error!(
                    "[TX-{:?}] Failed to begin SELECT transaction: {}",
                    select_tx_id,
                    e
                );
                return Err(anyhow!(e));
            }
        };

        tracing::debug!("[TX-{:?}] Executing SQL query: {}", select_tx_id, sql);
        let query_start = Instant::now();

        let pending_operations = match operation::Entity::find()
        .from_raw_sql(stmt)
        .all(&txn)
        .instrument(tracing::info_span!(
            "query_operations",
            tx_id = tx_id.to_string()
        ))
        .await
        {
            Ok(operations) => {
                tracing::debug!(
                    "[TX-{:?}] SQL query completed in {:?}ms, found {} operations",
                    select_tx_id,
                    query_start.elapsed().as_millis(),
                    operations.len()
                );

                // Commit the first transaction immediately
                let commit_start = Instant::now();
                if let Err(e) = txn
                .commit()
                .instrument(tracing::info_span!(
                    "commit_operations",
                    tx_id = tx_id.to_string()
                ))
                .await {
                    tracing::error!(
                        "[TX-{:?}] Failed to commit SELECT transaction: {}",
                        select_tx_id,
                        e
                    );
                    return Err(e.into());
                }
                tracing::debug!(
                    "[TX-{:?}] SELECT transaction committed in {:?}ms",
                    select_tx_id,
                    commit_start.elapsed().as_millis()
                );

                operations
            }
            Err(e) => {
                tracing::error!(
                    "[TX-{:?}] Failed to fetch pending operations: {}",
                    select_tx_id,
                    e
                );
                let _ = txn.rollback().await;
                return Err(e.into());
            }
        };

        // If no operations found, return early
        if pending_operations.is_empty() {
            tracing::debug!(
                "[TX-{:?}] No operations found, query_operations completed in {:?}ms",
                tx_id,
                start_time.elapsed().as_millis()
            );
            return Ok(vec![]);
        }

        // Second transaction: UPDATE the operations to processing status
        let update_tx_id = Uuid::new_v4();
        let update_start = Instant::now();
        tracing::debug!(
            "[TX-{:?}] Beginning UPDATE transaction for {} operations",
            update_tx_id,
            pending_operations.len()
        );

        let update_txn = match self.db.begin()
        .instrument(tracing::info_span!(
            "begin transaction for update operations",
            tx_id = update_tx_id.to_string()
        ))
        .await {
            Ok(txn) => {
                tracing::debug!(
                    "[TX-{:?}] UPDATE transaction started after {:?}ms",
                    update_tx_id,
                    update_start.elapsed().as_millis()
                );
                txn
            }
            Err(e) => {
                tracing::error!(
                    "[TX-{:?}] Failed to begin UPDATE transaction: {}",
                    update_tx_id,
                    e
                );
                return Err(anyhow!(e));
            }
        };

        let update_sql_start = Instant::now();
        let update_stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            r#"UPDATE operation SET status = $1 WHERE id = ANY($2) RETURNING id, operation_type, timestamp, status, next_retry, retry_count"#,
            vec![
                Value::SmallInt(Some(EntityStatus::Processing.to_id())),
                Value::Array(
                    ArrayType::String,
                    Some(Box::new(
                        pending_operations
                            .iter()
                            .map(|o| o.id.clone().into())
                            .collect(),
                    )),
                ),
            ],
        );

        match operation::Entity::find()
            .from_raw_sql(update_stmt)
            .all(&update_txn)
            .instrument(tracing::info_span!(
                "update_operations",
                tx_id = update_tx_id.to_string()
            ))
            .await
        {
            Ok(updated_list) => {
                tracing::debug!(
                    "[TX-{:?}] UPDATE operation completed in {:?}ms, updated {} operations",
                    update_tx_id,
                    update_sql_start.elapsed().as_millis(),
                    updated_list.len()
                );

                // Commit the transaction
                let commit_start = Instant::now();
                tracing::debug!("[TX-{:?}] Committing UPDATE operation transaction", update_tx_id);

                if let Err(e) = update_txn.commit().await {
                    tracing::error!(
                        "[TX-{:?}] Failed to commit UPDATE operation transaction: {}",
                        update_tx_id,
                        e
                    );
                    return Err(e.into());
                }

                tracing::info!(
                    "[TX-{:?}] UPDATE operation transaction committed in {:?}ms",
                    update_tx_id,
                    commit_start.elapsed().as_millis()
                );
                tracing::info!(
                    "[TX-{:?}] Complete query_operations operation took {:?}ms",
                    tx_id,
                    start_time.elapsed().as_millis()
                );

                Ok(updated_list)
            }
            Err(e) => {
                tracing::error!("[TX-{:?}] Failed to update operations: {}", update_tx_id, e);
                let _ = update_txn.rollback().await;
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
        tracing::debug!(
            "[TX-{:?}] Beginning transaction for set_operation_data",
            tx_id
        );

        let txn = match self.db.begin().await {
            Ok(txn) => {
                tracing::debug!(
                    "[TX-{:?}] Transaction started after {:?}ms",
                    tx_id,
                    start_time.elapsed().as_millis()
                );
                txn
            }
            Err(e) => {
                tracing::error!(
                    "[TX-{:?}] Failed to begin transaction after {:?}ms: {}",
                    tx_id,
                    start_time.elapsed().as_millis(),
                    e
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
                "Failed to delete existing stages for operation {}: {}",
                operation.id,
                e
            );
            let _ = txn.rollback().await;
            return Err(e.into());
        }

        // Update operation type and status
        let mut operation_model: operation::ActiveModel = operation.clone().into();
        if operation_data.operation_type.is_finalized() {
            operation_model.status = Set(EntityStatus::Finalized.to_id().into());
            // assume completed status
        }
        operation_model.operation_type = Set(Some(operation_data.operation_type.to_string()));

        if let Err(e) = operation_model
        .update(&txn)
        .instrument(tracing::info_span!("updating operation", tx_id = tx_id.to_string()))
        .await {
            tracing::error!("Failed to update operation status: {}", e);
            let _ = txn.rollback().await;

            return Err(e.into());
        }

        // Store operation stages
        for (stage_type, stage_data) in operation_data.stages.iter() {
            if let Some(data) = &stage_data.stage_data {
                let stage_model = operation_stage::ActiveModel {
                    id: NotSet,
                    operation_id: Set(operation.id.clone()),
                    stage_type_id: Set(stage_type.to_id()),
                    success: Set(data.success),
                    timestamp: Set(data.timestamp as i64),
                    note: Set(data.note.clone()),
                };

                // TODO: Update operation_stage if existing
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
                            "Successfully inserted stage {} for op_id {}",
                            inserted_stage.stage_type_id,
                            operation.id
                        );

                        // TODO: Remove existing transactions for this stage first (selected by stage_id)

                        // store transactions for this stage
                        for tx in data.transactions.iter() {
                            let tx_model = transaction::ActiveModel {
                                id: NotSet,
                                stage_id: Set(inserted_stage.id),
                                hash: Set(tx.hash.clone()),
                                blockchain_type: Set(tx.blockchain_type.to_string()),
                            };

                            match transaction::Entity::insert(tx_model).exec(&txn).await {
                                Ok(_) => tracing::debug!(
                                    "Successfully inserted transaction for stage_id {}",
                                    inserted_stage.id
                                ),
                                Err(e) => tracing::error!("Error inserting transaction: {:?}", e),
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Error inserting stage: {:?}", e);
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
            .instrument(tracing::info_span!("commiting insert transaction", tx_id = tx_id.to_string()))
            .await
        {
            Ok(_) => {
                tracing::info!(
                    "[TX-{:?}] Transaction committed in {:?}ms, total duration: {:?}ms",
                    tx_id,
                    commit_start.elapsed().as_millis(),
                    start_time.elapsed().as_millis()
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    "[TX-{:?}] Failed to commit transaction after {:?}ms: {}",
                    tx_id,
                    commit_start.elapsed().as_millis(),
                    e
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
        interval_model.next_retry =
            Set(Some(chrono::Utc::now().timestamp() + retry_after_delay_sec));
        interval_model.retry_count = Set(interval.retry_count + 1);
        interval_model.status = Set(EntityStatus::Pending.to_id()); // Reset status to pending

        match interval_model.update(self.db.as_ref()).await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!("Failed to update interval {} for retry: {}", interval.id, e);
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
        operation_model.next_retry =
            Set(Some(chrono::Utc::now().timestamp() + retry_after_delay_sec));
        operation_model.retry_count = Set(operation.retry_count + 1);
        operation_model.status = Set(EntityStatus::Pending.to_id().into()); // Reset status to pending

        match operation_model.update(self.db.as_ref()).await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(
                    "Failed to update operation {} for retry: {}",
                    operation.id,
                    e
                );
                Err(e.into())
            }
        }
    }

    pub async fn get_statistic(
        &self,
        historical_boundary: u64,
    ) -> anyhow::Result<DatabaseStatistic> {
        let first_timestamp = interval::Entity::find()
            .select_only()
            .column_as(interval::Column::Start.min(), "min_start")
            .into_tuple::<Option<i64>>()
            .one(self.db.as_ref());
        let last_timestamp = interval::Entity::find()
            .select_only()
            .column_as(interval::Column::End.max(), "max_end")
            .into_tuple::<Option<i64>>()
            .one(self.db.as_ref());
        let total_intervals = interval::Entity::find().count(self.db.as_ref());
        let failed_intervals = interval::Entity::find()
            .filter(interval::Column::Status.ne(EntityStatus::Finalized.to_id()))
            .filter(interval::Column::RetryCount.gt(0))
            .count(self.db.as_ref());
        let total_pending_intervals = interval::Entity::find()
            .filter(interval::Column::Status.eq(EntityStatus::Pending.to_id()))
            .count(self.db.as_ref());
        let historical_pending_intervals = interval::Entity::find()
            .filter(interval::Column::Status.eq(EntityStatus::Pending.to_id()))
            .filter(interval::Column::End.lt(historical_boundary))
            .count(self.db.as_ref());
        let realtime_pending_intervals = interval::Entity::find()
            .filter(interval::Column::Status.eq(EntityStatus::Pending.to_id()))
            .filter(interval::Column::Start.gte(historical_boundary))
            .count(self.db.as_ref());

        let sql1 = Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!(
                r#"SELECT SUM("end" - start)::BIGINT as sum FROM interval WHERE status = {} AND "end" < {}"#,
                EntityStatus::Finalized.to_id(),
                historical_boundary
            ),
        );
        let historical_processed_period = self.db.query_one(sql1);

        let sql2 = Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!(
                r#"SELECT SUM("end" - start)::BIGINT as sum FROM interval WHERE status = {} AND start >= {}"#,
                EntityStatus::Finalized.to_id(),
                historical_boundary
            ),
        );
        let realtime_processed_period = self.db.query_one(sql2);

        Ok(DatabaseStatistic {
            watermark: self.get_watermark().await?,
            first_timestamp: first_timestamp.await?.unwrap().unwrap_or(0) as u64,
            last_timestamp: last_timestamp.await?.unwrap().unwrap_or(0) as u64,
            total_intervals: total_intervals.await? as usize,
            failed_intervals: failed_intervals.await? as usize,
            total_pending_intervals: total_pending_intervals.await? as usize,
            historical_pending_intervals: historical_pending_intervals.await? as usize,
            realtime_pending_intervals: realtime_pending_intervals.await? as usize,
            historical_processed_period: historical_processed_period
                .await?
                .and_then(|row| row.try_get::<i64>("", "sum").ok())
                .unwrap_or(0) as u64,
            realtime_processed_period: realtime_processed_period
                .await?
                .and_then(|row| row.try_get::<i64>("", "sum").ok())
                .unwrap_or(0) as u64,
        })
    }

    pub async fn get_operations_with_stages(
        &self,
        id: &String,
    ) -> anyhow::Result<Option<(operation::Model, Vec<operation_stage::Model>)>> {
        let operation = operation::Entity::find()
            .filter(operation::Column::Id.eq(id))
            .find_with_related(operation_stage::Entity)
            .all(self.db.as_ref())
            .await?;

        Ok(operation.into_iter().next())
    }
}
