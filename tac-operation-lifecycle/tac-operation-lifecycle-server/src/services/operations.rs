use crate::proto::tac_service_server::TacService;
use base64::prelude::*;
use chrono::NaiveDateTime;
use std::sync::Arc;
use tac_operation_lifecycle_entity::{operation, operation_stage, transaction};
use tac_operation_lifecycle_logic::{
    client::models::profiling::OperationType,
    database::{OrderDirection, TacDatabase},
};
use tac_operation_lifecycle_proto::blockscout::tac_operation_lifecycle::v1::{
    GetOperationByTxHashRequest, GetOperationDetailsRequest, GetOperationsRequest,
    OperationBriefDetails, OperationDetails, OperationRelatedTransaction, OperationStage,
    OperationsFullResponse, OperationsResponse, Pagination, SearchOperationRequest,
};

pub struct OperationsService {
    db: Arc<TacDatabase>,
}

impl OperationsService {
    pub fn new(db: Arc<TacDatabase>) -> Self {
        Self { db }
    }
}

type OperationWithStages = (
    operation::Model,
    Vec<(operation_stage::Model, Vec<transaction::Model>)>,
);

const PAGE_SIZE: usize = 50;

impl OperationsService {
    pub fn convert_short_db_operation_into_response(
        db_data: Vec<operation::Model>,
    ) -> Vec<OperationBriefDetails> {
        db_data
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
            .collect()
    }

    pub fn convert_full_db_operation_into_response(
        (op, stages): OperationWithStages,
    ) -> OperationDetails {
        let op_type = match op.op_type {
            Some(t) => t.parse().unwrap_or(OperationType::ErrorType),
            _ => OperationType::Unknown,
        };
        OperationDetails {
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
                    items: OperationsService::convert_short_db_operation_into_response(operations),
                    next_page_params: last_timestamp.map(|ts| Pagination {
                        page_token: ts,
                        page_items: inner.page_items.unwrap_or(0) as u32 + PAGE_SIZE as u32,
                    }),
                }))
            }
            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }

    async fn search_operations(
        &self,
        request: tonic::Request<SearchOperationRequest>,
    ) -> Result<tonic::Response<OperationsResponse>, tonic::Status> {
        let q = request.into_inner().q;
        if is_generic_hash(&q) {
            // operation_id or tx_hash
            match self.db.get_brief_operation_by_id(&q).await {
                Ok(Some(op)) => Ok(tonic::Response::new(OperationsResponse {
                    items: OperationsService::convert_short_db_operation_into_response(
                        [op].to_vec(),
                    ),
                    next_page_params: None,
                })),
                _ => match self.db.get_brief_operations_by_tx_hash(&q).await {
                    Ok(res) => Ok(tonic::Response::new(OperationsResponse {
                        items: OperationsService::convert_short_db_operation_into_response(res),
                        next_page_params: None,
                    })),
                    _ => Ok(tonic::Response::new(OperationsResponse {
                        items: vec![],
                        next_page_params: None,
                    })),
                },
            }
        } else if is_tac_address(&q) || is_ton_address(&q) {
            // sender (TON-TAC format)
            // TODO: unimplemented for this version of the database.
            // The corresponding field doesn't exist for the operation entity.
            Ok(tonic::Response::new(OperationsResponse {
                items: vec![],
                next_page_params: None,
            }))
        } else {
            // unknown query string -> return void array without DB interacting
            Ok(tonic::Response::new(OperationsResponse {
                items: vec![],
                next_page_params: None,
            }))
        }
    }

    async fn get_operation_details(
        &self,
        request: tonic::Request<GetOperationDetailsRequest>,
    ) -> Result<tonic::Response<OperationDetails>, tonic::Status> {
        let inner = request.into_inner();

        match self.db.get_operation_by_id(&inner.operation_id).await {
            Ok(Some(full_data)) => Ok(tonic::Response::new(
                OperationsService::convert_full_db_operation_into_response(full_data),
            )),

            Ok(None) => Err(tonic::Status::not_found("cannot find operation id")),

            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }

    async fn get_operations_by_transaction(
        &self,
        request: tonic::Request<GetOperationByTxHashRequest>,
    ) -> std::result::Result<tonic::Response<OperationsFullResponse>, tonic::Status> {
        let inner = request.into_inner();

        match self.db.get_full_operations_by_tx_hash(&inner.tx_hash).await {
            Ok(operations) => Ok(tonic::Response::new(OperationsFullResponse {
                items: operations
                    .iter()
                    .map(|op| {
                        OperationsService::convert_full_db_operation_into_response(op.clone())
                    })
                    .collect(),
            })),

            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }
}

fn db_datetime_to_string(ts: NaiveDateTime) -> String {
    ts.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn is_generic_hash(q: &str) -> bool {
    q.starts_with("0x") && q.len() == 66 && q[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn is_tac_address(q: &str) -> bool {
    q.starts_with("0x") && q.len() == 42 && q[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn is_ton_address(q: &str) -> bool {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(q)
        .ok()
        .map(|bytes| bytes.len() == 36)
        .unwrap_or(false)
}
