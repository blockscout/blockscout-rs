use super::entity as job_queue;
use crate::entity::JobStatus;
use sea_orm::{
    prelude::Uuid,
    sea_query::{LockBehavior, LockType, SelectStatement},
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, DatabaseBackend, DbErr, EntityTrait, QueryFilter, QuerySelect,
    QueryTrait, Select, Statement,
};

pub async fn mark_as_success<C: ConnectionTrait>(
    db: &C,
    id: Uuid,
    message: Option<String>,
) -> Result<(), DbErr> {
    update_status(db, JobStatus::Success, id, message).await
}

pub async fn mark_as_error<C: ConnectionTrait>(
    db: &C,
    id: Uuid,
    message: Option<String>,
) -> Result<(), DbErr> {
    update_status(db, JobStatus::Error, id, message).await
}

async fn update_status<C: ConnectionTrait>(
    db: &C,
    new_status: JobStatus,
    id: Uuid,
    message: Option<String>,
) -> Result<(), DbErr> {
    job_queue::ActiveModel {
        id: Set(id),
        status: Set(new_status),
        log: Set(message),
        ..Default::default()
    }
    .update(db)
    .await
    .map(|_| ())
}

pub async fn next_job_id<C: ConnectionTrait>(db: &C) -> Result<Option<Uuid>, DbErr> {
    let noop_filter = |select| select;
    next_job_id_with_filter(db, noop_filter).await
}

pub async fn next_job_id_with_filter<C: ConnectionTrait, F>(
    db: &C,
    related_tables_filter: F,
) -> Result<Option<Uuid>, DbErr>
where
    F: Fn(Select<job_queue::Entity>) -> Select<job_queue::Entity>,
{
    let statement = next_job_id_statement(related_tables_filter);
    Ok(db.query_one(statement).await?.map(|query_result| {
        query_result
            .try_get_by::<Uuid, _>("id")
            .expect("error while try_get_by id")
    }))
}

fn next_job_id_statement<F>(related_tables_filter: F) -> Statement
where
    F: Fn(Select<job_queue::Entity>) -> Select<job_queue::Entity>,
{
    let subexpression = next_job_id_where_subexpression(related_tables_filter);
    let sql = format!(
        r#"
            UPDATE _job_queue
            SET status = 'in_process'
            WHERE id = ({})
            RETURNING _job_queue.id;
        "#,
        subexpression.to_string(sea_orm::sea_query::PostgresQueryBuilder)
    );

    Statement::from_string(DatabaseBackend::Postgres, sql.to_string())
}

/// Unwraps into (example):
/// `
/// SELECT
/// FROM _job_queue JOIN contract_address
///     ON _job_queue.id = contract_addresses._job_
/// WHERE _job_queue.status = 'waiting'
///     AND contract_addresses.chain_id = {}
/// LIMIT 1 FOR UPDATE SKIP LOCKED
/// `
fn next_job_id_where_subexpression<F>(related_tables_filter: F) -> SelectStatement
where
    F: Fn(Select<job_queue::Entity>) -> Select<job_queue::Entity>,
{
    let mut where_id_subexpression = job_queue::Entity::find()
        .select_only()
        .column(job_queue::Column::Id)
        .filter(job_queue::Column::Status.eq(job_queue::JobStatus::Waiting))
        .limit(1);
    where_id_subexpression = related_tables_filter(where_id_subexpression);
    where_id_subexpression
        .into_query()
        .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
        .to_owned()
}
