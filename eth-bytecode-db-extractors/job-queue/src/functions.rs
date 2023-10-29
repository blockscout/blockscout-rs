use super::entity as job_queue;
use crate::entity::JobStatus;
use sea_orm::{prelude::Uuid, ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DbErr};

pub async fn mark_as_success<C: ConnectionTrait>(
    db: &C,
    id: Uuid,
    message: Option<impl Into<String>>,
) -> Result<(), DbErr> {
    update_status(db, JobStatus::Success, id, message).await
}

pub async fn mark_as_error<C: ConnectionTrait>(
    db: &C,
    id: Uuid,
    message: Option<impl Into<String>>,
) -> Result<(), DbErr> {
    update_status(db, JobStatus::Error, id, message).await
}

async fn update_status<C: ConnectionTrait>(
    db: &C,
    new_status: JobStatus,
    id: Uuid,
    message: Option<impl Into<String>>,
) -> Result<(), DbErr> {
    job_queue::ActiveModel {
        id: Set(id),
        status: Set(new_status),
        log: Set(message.map(|msg| msg.into())),
        ..Default::default()
    }
    .update(db)
    .await
    .map(|_| ())
}

// pub async fn next<C: ConnectionTrait>(db: &C) -> Result<Option<workload_queue::Model>, DbErr> {
//     // Notice that we are looking only for contracts with given `chain_id`
//     let next_contract_address_sql = format!(
//         r#"
//             UPDATE workload_queue
//             SET status = 'in_process'
//             WHERE id = (SELECT id
//                                       FROM workload_queue JOIN contract_addresses
//                                         ON workload_queue.id = contract_addresses.workload_queue_id
//                                       WHERE workload_queue.status = 'waiting'
//                                         AND contract_addresses.chain_id = {}
//                                       LIMIT 1 FOR UPDATE SKIP LOCKED)
//             RETURNING contract_address;
//         "#,
//         self.chain_id
//     );
//
//     let next_contract_address_stmt = Statement::from_string(
//         DatabaseBackend::Postgres,
//         next_contract_address_sql.to_string(),
//     );
//
//     let next_contract_address = self
//         .db_client
//         .as_ref()
//         .query_one(next_contract_address_stmt)
//         .await
//         .context("querying for the next contract address")?
//         .map(|query_result| {
//             query_result
//                 .try_get_by::<Vec<u8>, _>("contract_address")
//                 .expect("error while try_get_by contract_address")
//         });
//
//     if let Some(contract_address) = next_contract_address {
//         let model = contract_addresses::Entity::find_by_id((
//             contract_address.clone(),
//             self.chain_id.into(),
//         ))
//             .one(self.db_client.as_ref())
//             .await
//             .expect("querying contract_address model failed")
//             .ok_or_else(|| {
//                 anyhow::anyhow!(
//                     "contract_address model does not exist for the contract: {}",
//                     Bytes::from(contract_address),
//                 )
//             })?;
//
//         return Ok(Some(model));
//     }
//
//     Ok(None)
// }
