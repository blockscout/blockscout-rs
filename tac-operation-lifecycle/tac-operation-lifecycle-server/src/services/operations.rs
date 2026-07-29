// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::proto::{v1, v2};
use chrono::NaiveDateTime;
use std::{collections::HashSet, sync::Arc};
use tac_operation_lifecycle_entity::{operation, operation_stage, transaction};
use tac_operation_lifecycle_logic::{
    database::{LogicPagination, TacDatabase},
    lifecycle::{project_v1_type, PROFILING_VERSION_V2},
};
use v1::tac_service_server::TacService as TacServiceV1;
use v2::tac_service_v2_server::TacServiceV2;

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

#[derive(Default)]
struct V2Lifecycle {
    r#type: Option<String>,
    success: Option<bool>,
    error_reason: Option<String>,
    finalized: Option<bool>,
    rollback: Option<bool>,
}

impl OperationsService {
    fn sender_parts(operation: &operation::Model) -> Option<(String, i32)> {
        match (
            operation.sender_address.clone(),
            operation.sender_blockchain.clone(),
        ) {
            (Some(addr), Some(chain)) => Some((
                addr,
                match chain.as_str() {
                    "Tac" => 0,
                    "Ton" => 1,
                    _ => 2,
                },
            )),
            _ => None,
        }
    }

    pub fn convert_short_db_operation_into_response(
        db_data: Vec<operation::Model>,
        insufficient_fee_ids: &HashSet<String>,
    ) -> Vec<v1::OperationBriefDetails> {
        db_data
            .into_iter()
            .map(|op| {
                let op_type = project_v1_type(
                    op.profiling_version,
                    op.op_type.as_deref(),
                    op.op_status.as_deref(),
                    op.finalized,
                    op.rollback,
                    insufficient_fee_ids.contains(&op.id),
                );

                v1::OperationBriefDetails {
                    operation_id: op.id.clone(),
                    r#type: op_type.to_id(),
                    timestamp: db_datetime_to_string(op.timestamp),
                    sender: Self::sender_parts(&op).map(|(address, blockchain)| {
                        v1::BlockchainAddress {
                            address,
                            blockchain,
                        }
                    }),
                }
            })
            .collect()
    }

    pub fn convert_full_db_operation_into_response(
        (op, stages): OperationWithStages,
    ) -> v1::OperationDetails {
        let insufficient_fee = stages.iter().any(|(stage, _)| {
            !stage.success
                && stage.note.as_ref().is_some_and(|note| {
                    let note = note.to_lowercase();
                    note.contains("insufficient") && note.contains("fee")
                })
        });
        let op_type = project_v1_type(
            op.profiling_version,
            op.op_type.as_deref(),
            op.op_status.as_deref(),
            op.finalized,
            op.rollback,
            insufficient_fee,
        );
        v1::OperationDetails {
            operation_id: op.id.clone(),
            r#type: op_type.to_id(),
            timestamp: db_datetime_to_string(op.timestamp),
            sender: Self::sender_parts(&op).map(|(address, blockchain)| v1::BlockchainAddress {
                address,
                blockchain,
            }),
            status_history: stages
                .iter()
                .map(|(s, txs)| v1::OperationStage {
                    r#type: s.stage_type_id as i32 - 1,
                    is_exist: true,
                    is_success: Some(s.success),
                    timestamp: (!txs.is_empty()).then(|| db_datetime_to_string(s.timestamp)),
                    transactions: txs
                        .iter()
                        .map(|tx| {
                            let blockchain_type = match tx.blockchain_type.as_str() {
                                "Tac" => 0,
                                "Ton" => 1,
                                _ => 2,
                            };
                            v1::OperationRelatedTransaction {
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

    pub fn extract_input_pagination(request: &v1::GetOperationsRequest) -> Option<LogicPagination> {
        let mut input_pagination = None;
        if let Some(pagination_token) = request.page_token {
            input_pagination = Some(LogicPagination {
                count: request.page_items.unwrap_or(0) as usize,
                earlier_timestamp: pagination_token,
            });
        }

        input_pagination
    }

    pub fn convert_logic_pagination(pagination: Option<LogicPagination>) -> Option<v1::Pagination> {
        pagination.map(|pag| v1::Pagination {
            page_token: pag.earlier_timestamp,
            page_items: pag.count as u32,
        })
    }

    async fn v1_insufficient_fee_ids(
        &self,
        operations: &[operation::Model],
    ) -> Result<HashSet<String>, tonic::Status> {
        let candidates: Vec<_> = operations
            .iter()
            .filter(|op| {
                op.profiling_version == PROFILING_VERSION_V2
                    && op.finalized == Some(false)
                    && op.op_status.as_deref() == Some("failed")
            })
            .map(|op| op.id.clone())
            .collect();
        self.db
            .get_insufficient_fee_operation_ids(&candidates)
            .await
            .map_err(map_db_error)
    }

    fn v2_lifecycle(operation: &operation::Model) -> V2Lifecycle {
        if operation.profiling_version != PROFILING_VERSION_V2 {
            return V2Lifecycle::default();
        }
        let success = match operation.op_status.as_deref() {
            Some("success") => Some(true),
            Some("failed") => Some(false),
            _ => None,
        };
        let error_reason = (success == Some(false))
            .then(|| operation.error_reason.clone())
            .flatten();
        V2Lifecycle {
            r#type: operation.op_type.clone(),
            success,
            error_reason,
            finalized: operation.finalized,
            rollback: operation.rollback,
        }
    }

    fn convert_short_v2(operation: operation::Model) -> v2::V2OperationBriefDetails {
        let V2Lifecycle {
            r#type,
            success,
            error_reason,
            finalized,
            rollback,
        } = Self::v2_lifecycle(&operation);
        v2::V2OperationBriefDetails {
            operation_id: operation.id.clone(),
            r#type,
            success,
            error_reason,
            finalized,
            rollback,
            timestamp: db_datetime_to_string(operation.timestamp),
            sender: Self::sender_parts(&operation).map(|(address, blockchain)| {
                v2::V2BlockchainAddress {
                    address,
                    blockchain,
                }
            }),
        }
    }

    fn convert_full_v2((operation, stages): OperationWithStages) -> v2::V2OperationDetails {
        let V2Lifecycle {
            r#type,
            success,
            error_reason,
            finalized,
            rollback,
        } = Self::v2_lifecycle(&operation);
        v2::V2OperationDetails {
            operation_id: operation.id.clone(),
            r#type,
            success,
            error_reason,
            finalized,
            rollback,
            timestamp: db_datetime_to_string(operation.timestamp),
            sender: Self::sender_parts(&operation).map(|(address, blockchain)| {
                v2::V2BlockchainAddress {
                    address,
                    blockchain,
                }
            }),
            status_history: stages
                .iter()
                .map(|(stage, transactions)| v2::V2OperationStage {
                    r#type: i32::from(stage.stage_type_id) - 1,
                    is_exist: true,
                    is_success: Some(stage.success),
                    timestamp: (!transactions.is_empty())
                        .then(|| db_datetime_to_string(stage.timestamp)),
                    transactions: transactions
                        .iter()
                        .map(|transaction| v2::V2OperationRelatedTransaction {
                            hash: transaction.hash.clone(),
                            r#type: match transaction.blockchain_type.as_str() {
                                "Tac" => 0,
                                "Ton" => 1,
                                _ => 2,
                            },
                        })
                        .collect(),
                    note: stage.note.clone(),
                })
                .collect(),
        }
    }
}

#[async_trait::async_trait]
impl TacServiceV1 for OperationsService {
    async fn get_operations(
        &self,
        request: tonic::Request<v1::GetOperationsRequest>,
    ) -> std::result::Result<tonic::Response<v1::OperationsResponse>, tonic::Status> {
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

        let insufficient_fee_ids = self.v1_insufficient_fee_ids(&operations).await?;
        Ok(tonic::Response::new(v1::OperationsResponse {
            items: Self::convert_short_db_operation_into_response(
                operations,
                &insufficient_fee_ids,
            ),
            next_page_params: Self::convert_logic_pagination(pagination),
        }))
    }

    async fn get_operation_details(
        &self,
        request: tonic::Request<v1::GetOperationDetailsRequest>,
    ) -> Result<tonic::Response<v1::OperationDetails>, tonic::Status> {
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
        request: tonic::Request<v1::GetOperationByTxHashRequest>,
    ) -> std::result::Result<tonic::Response<v1::OperationsFullResponse>, tonic::Status> {
        let inner = request.into_inner();

        match self.db.get_full_operations_by_tx_hash(&inner.tx_hash).await {
            Ok(operations) => Ok(tonic::Response::new(v1::OperationsFullResponse {
                items: operations
                    .iter()
                    .map(|op| Self::convert_full_db_operation_into_response(op.clone()))
                    .collect(),
            })),

            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }
}

#[async_trait::async_trait]
impl TacServiceV2 for OperationsService {
    async fn get_operations(
        &self,
        request: tonic::Request<v2::V2GetOperationsRequest>,
    ) -> Result<tonic::Response<v2::V2OperationsResponse>, tonic::Status> {
        let request = request.into_inner();
        let input_pagination = request.page_token.map(|page_token| LogicPagination {
            count: request.page_items.unwrap_or(0) as usize,
            earlier_timestamp: page_token,
        });
        let (operations, pagination) = match request.q {
            Some(query) => self
                .db
                .search_operations(&query, input_pagination)
                .await
                .map_err(map_db_error)?,
            None => self
                .db
                .get_operations(input_pagination)
                .await
                .map_err(map_db_error)?,
        };
        Ok(tonic::Response::new(v2::V2OperationsResponse {
            items: operations.into_iter().map(Self::convert_short_v2).collect(),
            next_page_params: pagination.map(|pagination| v2::V2Pagination {
                page_token: pagination.earlier_timestamp,
                page_items: pagination.count as u32,
            }),
        }))
    }

    async fn get_operation_details(
        &self,
        request: tonic::Request<v2::V2GetOperationDetailsRequest>,
    ) -> Result<tonic::Response<v2::V2OperationDetails>, tonic::Status> {
        match self
            .db
            .get_full_operation_by_id(&request.into_inner().operation_id)
            .await
        {
            Ok(Some(operation)) => Ok(tonic::Response::new(Self::convert_full_v2(operation))),
            Ok(None) => Err(tonic::Status::not_found("cannot find operation id")),
            Err(error) => Err(map_db_error(error)),
        }
    }

    async fn get_operations_by_transaction(
        &self,
        request: tonic::Request<v2::V2GetOperationByTxHashRequest>,
    ) -> Result<tonic::Response<v2::V2OperationsFullResponse>, tonic::Status> {
        let operations = self
            .db
            .get_full_operations_by_tx_hash(&request.into_inner().tx_hash)
            .await
            .map_err(map_db_error)?;
        Ok(tonic::Response::new(v2::V2OperationsFullResponse {
            items: operations.into_iter().map(Self::convert_full_v2).collect(),
        }))
    }
}

fn db_datetime_to_string(ts: NaiveDateTime) -> String {
    ts.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn map_db_error(err: anyhow::Error) -> tonic::Status {
    tonic::Status::internal(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tac_operation_lifecycle_entity::sea_orm_active_enums::StatusEnum;

    fn operation(profiling_version: i16, op_status: Option<&str>) -> operation::Model {
        let now = chrono::Utc::now().naive_utc();
        operation::Model {
            id: "op".to_string(),
            op_type: Some("TON-TAC".to_string()),
            profiling_version,
            op_status: op_status.map(str::to_string),
            error_reason: None,
            finalized: Some(true),
            rollback: Some(false),
            timestamp: now,
            next_retry: None,
            status: StatusEnum::Completed,
            retry_count: 0,
            inserted_at: now,
            updated_at: now,
            sender_address: None,
            sender_blockchain: None,
        }
    }

    #[test]
    fn v2_hides_canonical_fields_for_v1_source_rows() {
        let response = OperationsService::convert_short_v2(operation(1, Some("failed")));
        assert!(response.r#type.is_none());
        assert!(response.success.is_none());
        assert!(response.error_reason.is_none());
        assert!(response.finalized.is_none());
        assert!(response.rollback.is_none());
    }

    #[test]
    fn v2_exposes_independent_lifecycle_for_v2_rows() {
        let mut operation = operation(2, Some("failed"));
        operation.error_reason = Some("Insufficient Fee".to_string());
        let response = OperationsService::convert_short_v2(operation);
        assert_eq!(response.r#type.as_deref(), Some("TON-TAC"));
        assert_eq!(response.success, Some(false));
        assert_eq!(response.error_reason.as_deref(), Some("Insufficient Fee"));
        assert_eq!(response.finalized, Some(true));
        assert_eq!(response.rollback, Some(false));
    }

    #[test]
    fn v2_maps_only_known_operation_statuses_to_success() {
        let mut successful_operation = operation(2, Some("success"));
        successful_operation.error_reason = Some("must stay hidden".to_string());
        let successful = OperationsService::convert_short_v2(successful_operation);
        let mut unknown_operation = operation(2, Some("pending"));
        unknown_operation.error_reason = Some("must stay hidden".to_string());
        let unknown = OperationsService::convert_short_v2(unknown_operation);
        let absent = OperationsService::convert_short_v2(operation(2, None));

        assert_eq!(successful.success, Some(true));
        assert_eq!(successful.error_reason, None);
        assert_eq!(unknown.success, None);
        assert_eq!(unknown.error_reason, None);
        assert_eq!(absent.success, None);
        assert_eq!(absent.error_reason, None);

        let successful_json =
            serde_json::to_value(successful).expect("v2 operation response must serialize");
        assert_eq!(successful_json["type"], "TON-TAC");
        assert_eq!(successful_json["success"], true);

        let unknown_json =
            serde_json::to_value(unknown).expect("v2 operation response must serialize");
        assert_eq!(unknown_json["success"], serde_json::Value::Null);
        assert_eq!(unknown_json["error_reason"], serde_json::Value::Null);
    }
}
