use std::sync::Arc;

use chrono::NaiveDateTime;
use tac_operation_lifecycle_entity::{operation, operation_stage, transaction};
use tac_operation_lifecycle_logic::{
    client::models::profiling::OperationType,
    database::{OrderDirection, TacDatabase},
};
use tac_operation_lifecycle_proto::blockscout::tac_operation_lifecycle::v1::{
    GetOperationByTxHashRequest, GetOperationDetailsRequest, GetOperationsRequest,
    OperationBriefDetails, OperationDetails, OperationRelatedTransaction, OperationStage,
    OperationsResponse, Pagination,
};

use crate::proto::tac_service_server::TacService;

pub struct OperationsService {
    db: Arc<TacDatabase>,
}

impl OperationsService {
    pub fn new(db: Arc<TacDatabase>) -> Self {
        Self { db }
    }
}

type OperationWithStages = Option<(
    operation::Model,
    Vec<(operation_stage::Model, Vec<transaction::Model>)>,
)>;

impl OperationsService {
    pub fn create_full_operation_response(
        db_data: anyhow::Result<OperationWithStages>,
    ) -> Result<tonic::Response<OperationDetails>, tonic::Status> {
        match db_data {
            Ok(Some((op, stages))) => {
                let op_type = match op.op_type {
                    Some(t) => t.parse().unwrap_or(OperationType::ErrorType),
                    _ => OperationType::Unknown,
                };
                Ok(tonic::Response::new(OperationDetails {
                    operation_id: op.id,
                    r#type: op_type.to_id(),
                    timestamp: db_datetime_to_string(op.timestamp),
                    sender: None,
                    status_history: stages
                        .iter()
                        .map(|(s, txs)| OperationStage {
                            r#type: s.stage_type_id as i32 - 1,
                            is_exist: true,
                            is_success: Some(s.success),
                            timestamp: Some(db_datetime_to_string(s.timestamp)),
                            transactions: txs
                                .iter()
                                .map(|tx| {
                                    let blockchain_type = match tx.blockchain_type.as_str() {
                                        "Tac" => 0,
                                        "Ton" => 1,
                                        _ => 2,
                                    };
                                    OperationRelatedTransaction {
                                        hash: tx.hash.clone(),
                                        r#type: blockchain_type,
                                    }
                                })
                                .collect(),
                            note: s.note.clone(),
                        })
                        .collect(),
                }))
            }

            Ok(None) => Err(tonic::Status::not_found("cannot find operation id")),

            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }
}

#[async_trait::async_trait]
impl TacService for OperationsService {
    async fn get_operations(
        &self,
        request: tonic::Request<GetOperationsRequest>,
    ) -> std::result::Result<tonic::Response<OperationsResponse>, tonic::Status> {
        let inner = request.into_inner();

        const PAGE_SIZE: usize = 50;

        match self
            .db
            .get_operations(PAGE_SIZE, inner.page_token, OrderDirection::LatestFirst)
            .await
        {
            Ok(operations) => {
                let last_timestamp = operations
                    .last()
                    .map(|op| op.timestamp.and_utc().timestamp() as u64);

                Ok(tonic::Response::new(OperationsResponse {
                    items: operations
                        .into_iter()
                        .map(|op| {
                            let op_type = match op.op_type {
                                Some(t) => t.parse().unwrap_or(OperationType::ErrorType),
                                _ => OperationType::Unknown,
                            };
                            OperationBriefDetails {
                                operation_id: op.id,
                                r#type: op_type.to_id(),
                                timestamp: db_datetime_to_string(op.timestamp),
                                sender: None,
                            }
                        })
                        .collect(),
                    next_page_params: last_timestamp.map(|ts| Pagination {
                        page_token: ts,
                        page_items: inner.page_items.unwrap_or(0) as u32 + PAGE_SIZE as u32,
                    }),
                }))
            }
            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }

    async fn get_operation_details(
        &self,
        request: tonic::Request<GetOperationDetailsRequest>,
    ) -> Result<tonic::Response<OperationDetails>, tonic::Status> {
        let inner = request.into_inner();

        let db_resp = self.db.get_operation_by_id(&inner.operation_id).await;

        OperationsService::create_full_operation_response(db_resp)
    }

    async fn get_operation_by_transaction(
        &self,
        request: tonic::Request<GetOperationByTxHashRequest>,
    ) -> std::result::Result<tonic::Response<OperationDetails>, tonic::Status> {
        let inner = request.into_inner();

        let db_resp = self.db.get_operation_by_tx_hash(&inner.tx_hash).await;

        OperationsService::create_full_operation_response(db_resp)
    }
}

fn db_datetime_to_string(ts: NaiveDateTime) -> String {
    ts.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
