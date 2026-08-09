// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::proto::{v1, v2};
use chrono::NaiveDateTime;
use std::{collections::HashSet, sync::Arc};
use tac_operation_lifecycle_entity::{operation, operation_stage, transaction};
use tac_operation_lifecycle_logic::{
    database::{LogicPagination, TacDatabase},
    lifecycle::{
        note_indicates_insufficient_fee, project_v1_type, PROFILING_VERSION_V1,
        PROFILING_VERSION_V2,
    },
};
use v1::tac_service_server::TacService as TacServiceV1;
use v2::tac_service_v2_server::TacServiceV2;

/// Upper bound on a published `error_reason`, in characters.
///
/// The database keeps whatever the stage note yielded. Anything longer than
/// this is a raw upstream payload (serialized revert data, a whole message
/// body) that the API cannot present as a short label, so it is withheld —
/// the full note is still available per stage in `status_history`.
const MAX_ERROR_REASON_LEN: usize = 16;

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

struct V2Lifecycle {
    r#type: v2::V2OperationType,
    status: v2::V2OperationStatus,
    error_reason: Option<String>,
    rollback: bool,
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
                && stage
                    .note
                    .as_deref()
                    .is_some_and(note_indicates_insufficient_fee)
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
                op.profiling_version == Some(PROFILING_VERSION_V2)
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

    /// Projects stored lifecycle facts into the public v2 view.
    ///
    /// This is an intentional product projection, not a raw field dump. The
    /// decisions below are deliberate and should not be "corrected" into a
    /// one-to-one mapping of the upstream Stage Profiler fields:
    ///
    /// 1. `failed` ignores finality. Upstream `finalized` tells *the indexer*
    ///    whether to keep re-requesting an operation; it is not a public
    ///    concept, and the v2 messages reserve the field. A user cannot act on
    ///    a failed operation, and "failed and pending at the same time" is not
    ///    a state worth surfacing, so a failed operation reads as `failed` as
    ///    soon as upstream says so.
    /// 2. `pending` is the fallback. It covers rows with no profiling data yet
    ///    (`profiling_version IS NULL`) and successful-but-not-yet-final
    ///    operations. A v2 row never carries `finalized=true` without an
    ///    `op_status`, because both columns are written together from one
    ///    successful `/v2/stage-profiling` response.
    /// 3. Legacy `profiling_version = 1` rows are mapped from the old
    ///    overloaded `op_type` instead of being reported as unknown. The old
    ///    model could not express a final failure without rollback, so a legacy
    ///    route reads as `success` - which matches what the current frontend
    ///    already shows. Legacy rows are transitional: the v2 backfill worker
    ///    converts them, so this path is a compatibility bridge, not a
    ///    long-lived contract.
    /// 4. `error_reason` is a short label, not the raw failure text. It is
    ///    published only for a failed v2 row, and only when the stored value
    ///    fits [`MAX_ERROR_REASON_LEN`]; longer values stay in the database and
    ///    are readable per stage through `status_history`.
    fn v2_lifecycle(operation: &operation::Model) -> V2Lifecycle {
        let status = match operation.profiling_version {
            None => v2::V2OperationStatus::Pending,
            Some(PROFILING_VERSION_V1) => match operation.op_type.as_deref() {
                Some("ROLLBACK" | "INSUFFICIENT-FEE" | "INSUFFICIENT_FEE") => {
                    v2::V2OperationStatus::Failed
                }
                Some("TON-TAC" | "TON-TAC-TON" | "TAC-TON") => v2::V2OperationStatus::Success,
                _ => v2::V2OperationStatus::Pending,
            },
            Some(PROFILING_VERSION_V2) => {
                match (operation.op_status.as_deref(), operation.finalized) {
                    (Some("failed"), _) => v2::V2OperationStatus::Failed,
                    (Some("success"), Some(true)) => v2::V2OperationStatus::Success,
                    _ => v2::V2OperationStatus::Pending,
                }
            }
            Some(_) => v2::V2OperationStatus::Pending,
        };
        let is_v2 = operation.profiling_version == Some(PROFILING_VERSION_V2);
        let error_reason = (is_v2 && status == v2::V2OperationStatus::Failed)
            .then(|| operation.error_reason.clone())
            .flatten()
            .filter(|reason| reason.chars().count() <= MAX_ERROR_REASON_LEN);
        V2Lifecycle {
            r#type: match operation.op_type.as_deref() {
                None | Some("UNKNOWN") => v2::V2OperationType::Unknown,
                Some("TON-TAC-TON") => v2::V2OperationType::TonTacTon,
                Some("TAC-TON") => v2::V2OperationType::TacTon,
                Some("TON-TAC") => v2::V2OperationType::TonTac,
                Some(_) => v2::V2OperationType::Unknown,
            },
            status,
            error_reason,
            rollback: operation.op_type.as_deref() == Some("ROLLBACK")
                || (is_v2 && operation.rollback == Some(true)),
        }
    }

    fn convert_short_v2(operation: operation::Model) -> v2::V2OperationBriefDetails {
        let V2Lifecycle {
            r#type,
            status,
            error_reason,
            rollback,
        } = Self::v2_lifecycle(&operation);
        v2::V2OperationBriefDetails {
            operation_id: operation.id.clone(),
            r#type: r#type as i32,
            status: status as i32,
            error_reason,
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
            status,
            error_reason,
            rollback,
        } = Self::v2_lifecycle(&operation);
        v2::V2OperationDetails {
            operation_id: operation.id.clone(),
            r#type: r#type as i32,
            status: status as i32,
            error_reason,
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

    fn operation(
        profiling_version: Option<i16>,
        op_type: Option<&str>,
        op_status: Option<&str>,
        finalized: Option<bool>,
    ) -> operation::Model {
        let now = chrono::Utc::now().naive_utc();
        operation::Model {
            id: "op".to_string(),
            op_type: op_type.map(str::to_string),
            profiling_version,
            op_status: op_status.map(str::to_string),
            error_reason: None,
            finalized,
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
    fn v2_status_projection_covers_profiling_versions() {
        let cases = [
            (None, None, None, None, v2::V2OperationStatus::Pending),
            (
                None,
                Some("TON-TAC"),
                Some("success"),
                Some(true),
                v2::V2OperationStatus::Pending,
            ),
            (Some(1), None, None, None, v2::V2OperationStatus::Pending),
            (
                Some(1),
                Some("PENDING"),
                None,
                None,
                v2::V2OperationStatus::Pending,
            ),
            (
                Some(1),
                Some("ROLLBACK"),
                None,
                None,
                v2::V2OperationStatus::Failed,
            ),
            (
                Some(1),
                Some("INSUFFICIENT-FEE"),
                None,
                None,
                v2::V2OperationStatus::Failed,
            ),
            (
                Some(1),
                Some("INSUFFICIENT_FEE"),
                None,
                None,
                v2::V2OperationStatus::Failed,
            ),
            (
                Some(1),
                Some("TON-TAC"),
                None,
                None,
                v2::V2OperationStatus::Success,
            ),
            (
                Some(1),
                Some("TON-TAC-TON"),
                None,
                None,
                v2::V2OperationStatus::Success,
            ),
            (
                Some(1),
                Some("TAC-TON"),
                None,
                None,
                v2::V2OperationStatus::Success,
            ),
            (
                Some(2),
                Some("TON-TAC"),
                None,
                Some(false),
                v2::V2OperationStatus::Pending,
            ),
            (
                Some(2),
                Some("TON-TAC"),
                None,
                Some(true),
                v2::V2OperationStatus::Pending,
            ),
            (
                Some(2),
                Some("TON-TAC"),
                Some("success"),
                Some(false),
                v2::V2OperationStatus::Pending,
            ),
            (
                Some(2),
                Some("TON-TAC"),
                Some("failed"),
                Some(false),
                v2::V2OperationStatus::Failed,
            ),
            (
                Some(2),
                Some("TON-TAC"),
                Some("failed"),
                Some(true),
                v2::V2OperationStatus::Failed,
            ),
            (
                Some(2),
                Some("TON-TAC"),
                Some("success"),
                Some(true),
                v2::V2OperationStatus::Success,
            ),
        ];

        for (profiling_version, op_type, op_status, finalized, expected) in cases {
            let response = OperationsService::convert_short_v2(operation(
                profiling_version,
                op_type,
                op_status,
                finalized,
            ));
            assert_eq!(response.status, expected as i32);
        }
    }

    #[test]
    fn v2_type_projection_covers_known_and_unknown_values() {
        let cases = [
            (None, v2::V2OperationType::Unknown),
            (Some("UNKNOWN"), v2::V2OperationType::Unknown),
            (Some("TON-TAC-TON"), v2::V2OperationType::TonTacTon),
            (Some("TAC-TON"), v2::V2OperationType::TacTon),
            (Some("TON-TAC"), v2::V2OperationType::TonTac),
            (Some("PENDING"), v2::V2OperationType::Unknown),
            (Some("ROLLBACK"), v2::V2OperationType::Unknown),
            (Some("INSUFFICIENT-FEE"), v2::V2OperationType::Unknown),
            (Some("INSUFFICIENT_FEE"), v2::V2OperationType::Unknown),
            (Some("ERROR"), v2::V2OperationType::Unknown),
            (Some("NEW-ROUTE"), v2::V2OperationType::Unknown),
        ];

        for (op_type, expected) in cases {
            let response = OperationsService::convert_short_v2(operation(
                Some(PROFILING_VERSION_V1),
                op_type,
                None,
                None,
            ));
            assert_eq!(response.r#type, expected as i32);
        }
    }

    #[test]
    fn v2_uses_unknown_type_until_operation_type_is_known() {
        let response = OperationsService::convert_short_v2(operation(None, None, None, None));

        assert_eq!(response.r#type, v2::V2OperationType::Unknown as i32);
        assert_eq!(response.status, v2::V2OperationStatus::Pending as i32);
        assert!(response.error_reason.is_none());
        assert!(!response.rollback);

        let response =
            serde_json::to_value(response).expect("v2 brief operation response must serialize");
        assert_eq!(response["type"], "UNKNOWN");
        assert_eq!(response["status"], "pending");
        assert_eq!(response["rollback"], false);
    }

    #[test]
    fn v2_exposes_failed_error_reason_and_rollback_for_v2_rows() {
        let mut operation = operation(Some(2), Some("TON-TAC"), Some("failed"), Some(false));
        operation.error_reason = Some("Insufficient Fee".to_string());
        operation.rollback = Some(true);
        let response = OperationsService::convert_short_v2(operation);

        assert_eq!(response.r#type, v2::V2OperationType::TonTac as i32);
        assert_eq!(response.status, v2::V2OperationStatus::Failed as i32);
        assert_eq!(response.error_reason.as_deref(), Some("Insufficient Fee"));
        assert!(response.rollback);
    }

    #[test]
    fn v2_withholds_error_reasons_longer_than_the_limit() {
        let failed_with = |reason: &str| {
            let mut model = operation(
                Some(PROFILING_VERSION_V2),
                Some("TON-TAC"),
                Some("failed"),
                Some(true),
            );
            model.error_reason = Some(reason.to_string());
            OperationsService::convert_short_v2(model)
        };

        let at_limit = "x".repeat(MAX_ERROR_REASON_LEN);
        let above_limit = "x".repeat(MAX_ERROR_REASON_LEN + 1);

        assert_eq!(
            failed_with(&at_limit).error_reason.as_deref(),
            Some(at_limit.as_str())
        );
        assert_eq!(failed_with(&above_limit).error_reason, None);
        assert_eq!(
            failed_with("ProxyCallError: default error").error_reason,
            None
        );
        // withholding the label does not change the projected status
        assert_eq!(
            failed_with(&above_limit).status,
            v2::V2OperationStatus::Failed as i32
        );
    }

    #[test]
    fn v2_rollback_is_true_only_when_confirmed() {
        let legacy_rollback = OperationsService::convert_short_v2(operation(
            Some(PROFILING_VERSION_V1),
            Some("ROLLBACK"),
            None,
            None,
        ));
        let mut legacy_route = operation(Some(PROFILING_VERSION_V1), Some("TON-TAC"), None, None);
        legacy_route.rollback = Some(true);
        let legacy_route = OperationsService::convert_short_v2(legacy_route);
        let mut v2_rollback = operation(
            Some(PROFILING_VERSION_V2),
            Some("TON-TAC"),
            Some("failed"),
            Some(true),
        );
        v2_rollback.rollback = Some(true);
        let v2_rollback = OperationsService::convert_short_v2(v2_rollback);
        let mut unprofiled = operation(None, None, None, None);
        unprofiled.rollback = Some(true);
        let unprofiled = OperationsService::convert_short_v2(unprofiled);

        assert!(legacy_rollback.rollback);
        assert!(!legacy_route.rollback);
        assert!(v2_rollback.rollback);
        assert!(!unprofiled.rollback);
    }

    #[test]
    fn full_v2_response_uses_the_same_projection_without_legacy_fields() {
        let response = OperationsService::convert_full_v2((
            operation(Some(2), None, Some("success"), Some(true)),
            Vec::new(),
        ));
        let response =
            serde_json::to_value(response).expect("v2 operation response must serialize");

        assert_eq!(response["type"], "UNKNOWN");
        assert_eq!(response["status"], "success");
        assert_eq!(response["rollback"], false);
        assert!(response.get("success").is_none());
        assert!(response.get("finalized").is_none());
    }
}
