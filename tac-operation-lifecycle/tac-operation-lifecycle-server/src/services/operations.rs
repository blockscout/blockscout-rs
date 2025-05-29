use crate::proto::tac_service_server::TacService;
use chrono::NaiveDateTime;
use std::sync::Arc;
use tac_operation_lifecycle_entity::{operation, operation_stage, transaction};
use tac_operation_lifecycle_logic::{
    client::models::profiling::OperationType,
    database::{LogicPagination, TacDatabase},
};
use tac_operation_lifecycle_proto::blockscout::tac_operation_lifecycle::v1::{
    BlockchainAddress, GetOperationByTxHashRequest, GetOperationDetailsRequest,
    GetOperationsRequest, OperationBriefDetails, OperationDetails, OperationRelatedTransaction,
    OperationStage, OperationsFullResponse, OperationsResponse, Pagination,
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

impl OperationsService {
    fn extract_sender(operation: &operation::Model) -> Option<BlockchainAddress> {
        match (
            operation.sender_address.clone(),
            operation.sender_blockchain.clone(),
        ) {
            (Some(addr), Some(chain)) => Some(BlockchainAddress {
                address: addr,
                blockchain: match chain.as_str() {
                    "Tac" => 0,
                    "Ton" => 1,
                    _ => 2,
                },
            }),
            _ => None,
        }
    }

    pub fn convert_short_db_operation_into_response(
        db_data: Vec<operation::Model>,
    ) -> Vec<OperationBriefDetails> {
        db_data
            .into_iter()
            .map(|op| {
                let op_type = match op.op_type.clone() {
                    Some(t) => t.parse().unwrap_or(OperationType::ErrorType),
                    _ => OperationType::Unknown,
                };

                OperationBriefDetails {
                    operation_id: op.id.clone(),
                    r#type: op_type.to_id(),
                    timestamp: db_datetime_to_string(op.timestamp),
                    sender: Self::extract_sender(&op),
                }
            })
            .collect()
    }

    pub fn convert_full_db_operation_into_response(
        (op, stages): OperationWithStages,
    ) -> OperationDetails {
        let op_type = match op.op_type.clone() {
            Some(t) => t.parse().unwrap_or(OperationType::ErrorType),
            _ => OperationType::Unknown,
        };
        OperationDetails {
            operation_id: op.id.clone(),
            r#type: op_type.to_id(),
            timestamp: db_datetime_to_string(op.timestamp),
            sender: Self::extract_sender(&op),
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

    pub fn extract_input_pagination(request: &GetOperationsRequest) -> Option<LogicPagination> {
        let mut input_pagination = None;
        if let Some(pagination_token) = request.page_token {
            input_pagination = Some(LogicPagination {
                count: request.page_items.unwrap_or(0) as usize,
                earlier_timestamp: pagination_token,
            });
        }

        input_pagination
    }

    pub fn convert_logic_pagination(pagination: Option<LogicPagination>) -> Option<Pagination> {
        pagination.map(|pag| Pagination {
            page_token: pag.earlier_timestamp,
            page_items: pag.count as u32,
        })
    }
}

#[async_trait::async_trait]
impl TacService for OperationsService {
    async fn get_operations(
        &self,
        request: tonic::Request<GetOperationsRequest>,
    ) -> std::result::Result<tonic::Response<OperationsResponse>, tonic::Status> {
        let inner = request.into_inner();

        let input_pagination = Self::extract_input_pagination(&inner);

        let (operations, pagination) = match inner.q {
            Some(q) => {
                // find operations by query
                self.db
                    .search_operations(&q, input_pagination)
                    .await
                    .map_err(map_db_error)?
            }
            None => {
                // simple operations list with pagination
                self.db
                    .get_operations(input_pagination)
                    .await
                    .map_err(map_db_error)?
            }
        };

        let next_page_params = Self::convert_logic_pagination(pagination);

        Ok(tonic::Response::new(OperationsResponse {
            items: Self::convert_short_db_operation_into_response(operations),
            next_page_params,
        }))
    }

    async fn get_operation_details(
        &self,
        request: tonic::Request<GetOperationDetailsRequest>,
    ) -> Result<tonic::Response<OperationDetails>, tonic::Status> {
        let inner = request.into_inner();

        match self.db.get_full_operation_by_id(&inner.operation_id).await {
            Ok(Some(full_data)) => Ok(tonic::Response::new(
                Self::convert_full_db_operation_into_response(full_data),
            )),

            Ok(None) => Err(tonic::Status::not_found("cannot find operation id")),

            Err(e) => Err(map_db_error(e)),
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
                    .map(|op| Self::convert_full_db_operation_into_response(op.clone()))
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

fn map_db_error(err: anyhow::Error) -> tonic::Status {
    tonic::Status::internal(err.to_string())
}
