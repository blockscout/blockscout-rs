// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::client::models::profiling::{
    LegacyOperationType, OperationRoute, OperationStatus, SourceOperationData, Stage, StageType,
};
use serde_json::Value;
use std::{collections::HashMap, str::FromStr};

pub const PROFILING_VERSION_V1: i16 = 1;
pub const PROFILING_VERSION_V2: i16 = 2;
pub const INSUFFICIENT_FEE_ERROR_REASON: &str = "Insufficient Fee";

pub fn has_insufficient_fee_stages<'a>(stages: impl IntoIterator<Item = &'a Stage>) -> bool {
    stages
        .into_iter()
        .filter_map(|stage| stage.stage_data.as_ref())
        .any(|stage_data| {
            !stage_data.success
                && stage_data.note.as_ref().is_some_and(|note| {
                    let note = note.to_lowercase();
                    note.contains("insufficient") && note.contains("fee")
                })
        })
}

pub fn derive_v1_source_type(
    operation_type: &LegacyOperationType,
    stages: &HashMap<crate::client::models::profiling::StageType, Stage>,
) -> LegacyOperationType {
    if operation_type == &LegacyOperationType::Pending
        && has_insufficient_fee_stages(stages.values())
    {
        LegacyOperationType::InsufficientFee
    } else {
        operation_type.clone()
    }
}

pub fn derive_operation_error_reason(data: &SourceOperationData) -> Option<String> {
    let SourceOperationData::V2(data) = data else {
        return None;
    };
    if data.status != Some(OperationStatus::Failed) {
        return None;
    }
    if has_insufficient_fee_stages(data.stages.values()) {
        return Some(INSUFFICIENT_FEE_ERROR_REASON.to_string());
    }

    data.stages
        .iter()
        .filter_map(|(stage_type, stage)| {
            let stage = stage.stage_data.as_ref()?;
            if stage.success {
                return None;
            }
            let reason = stage.note.as_deref().and_then(error_reason_from_note)?;
            Some((stage_order(stage_type), reason))
        })
        .max_by_key(|(order, _)| *order)
        .map(|(_, reason)| reason)
}

fn stage_order(stage_type: &StageType) -> u8 {
    match stage_type {
        StageType::CollectedInTAC => 1,
        StageType::IncludedInTACConsensus => 2,
        StageType::ExecutedInTAC => 3,
        StageType::CollectedInTON => 4,
        StageType::IncludedInTONConsensus => 5,
        StageType::ExecutedInTON => 6,
    }
}

fn error_reason_from_note(note: &str) -> Option<String> {
    fn non_empty(value: &str) -> Option<String> {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    }

    let note = note.trim();
    if note.is_empty() {
        return None;
    }
    let Ok(value) = serde_json::from_str::<Value>(note) else {
        return Some(note.to_string());
    };
    match value {
        Value::String(value) => non_empty(&value),
        Value::Object(values) => ["internalMsg", "content", "errorName", "internalBytesError"]
            .into_iter()
            .find_map(|key| values.get(key)?.as_str().and_then(non_empty)),
        Value::Null => None,
        value => non_empty(&value.to_string()),
    }
}

pub fn project_v1_type(
    profiling_version: Option<i16>,
    stored_type: Option<&str>,
    op_status: Option<&str>,
    finalized: Option<bool>,
    rollback: Option<bool>,
    has_insufficient_fee: bool,
) -> LegacyOperationType {
    if profiling_version != Some(PROFILING_VERSION_V2) {
        return stored_type
            .and_then(|value| LegacyOperationType::from_str(value).ok())
            .unwrap_or(LegacyOperationType::Unknown);
    }

    match finalized {
        Some(false) if op_status == Some("failed") && has_insufficient_fee => {
            LegacyOperationType::InsufficientFee
        }
        Some(false) => LegacyOperationType::Pending,
        Some(true) if op_status == Some("failed") && rollback == Some(true) => {
            LegacyOperationType::Rollback
        }
        Some(true) => match stored_type.and_then(|value| OperationRoute::from_str(value).ok()) {
            Some(OperationRoute::TonTacTon) => LegacyOperationType::TonTacTon,
            Some(OperationRoute::TacTon) => LegacyOperationType::TacTon,
            Some(OperationRoute::TonTac) => LegacyOperationType::TonTac,
            Some(OperationRoute::Unknown) => LegacyOperationType::Unknown,
            Some(OperationRoute::Unrecognized(_)) | None => LegacyOperationType::ErrorType,
        },
        None => LegacyOperationType::ErrorType,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::models::profiling::{StageData, V2OperationData};

    fn failed_stage(note: &str) -> Stage {
        Stage {
            exists: true,
            stage_data: Some(StageData {
                success: false,
                timestamp: 1,
                transactions: None,
                note: Some(note.to_string()),
            }),
        }
    }

    fn v2_data(
        status: Option<OperationStatus>,
        stages: HashMap<StageType, Stage>,
    ) -> SourceOperationData {
        SourceOperationData::V2(V2OperationData {
            operation_type: OperationRoute::TonTac,
            status,
            finalized: true,
            rollback: false,
            meta_info: None,
            stages,
        })
    }

    #[test]
    fn v2_projection_ignores_technical_status() {
        assert_eq!(
            project_v1_type(
                Some(2),
                Some("TON-TAC"),
                None,
                Some(false),
                Some(false),
                false,
            ),
            LegacyOperationType::Pending
        );
    }

    #[test]
    fn failed_is_required_for_special_v2_projection() {
        assert_eq!(
            project_v1_type(
                Some(2),
                Some("TON-TAC"),
                Some("failed"),
                Some(false),
                Some(true),
                true
            ),
            LegacyOperationType::InsufficientFee
        );
        assert_eq!(
            project_v1_type(
                Some(2),
                Some("TON-TAC"),
                Some("success"),
                Some(false),
                Some(true),
                true
            ),
            LegacyOperationType::Pending
        );
        assert_eq!(
            project_v1_type(
                Some(2),
                Some("TON-TAC"),
                Some("failed"),
                Some(true),
                Some(true),
                false
            ),
            LegacyOperationType::Rollback
        );
    }

    #[test]
    fn legacy_projection_returns_stored_overloaded_type() {
        assert_eq!(
            project_v1_type(Some(1), Some("INSUFFICIENT-FEE"), None, None, None, false,),
            LegacyOperationType::InsufficientFee
        );
    }

    #[test]
    fn unprofiled_projection_is_unknown() {
        assert_eq!(
            project_v1_type(None, None, None, None, None, false),
            LegacyOperationType::Unknown
        );
    }

    #[test]
    fn v2_projection_truth_table_covers_nonfinal_and_final_failures() {
        let cases = [
            (
                ("TON-TAC", Some("failed"), Some(false), Some(false), true),
                LegacyOperationType::InsufficientFee,
            ),
            (
                ("TON-TAC", Some("failed"), Some(false), Some(false), false),
                LegacyOperationType::Pending,
            ),
            (
                ("TON-TAC", Some("success"), Some(false), Some(true), true),
                LegacyOperationType::Pending,
            ),
            (
                ("TON-TAC", Some("failed"), Some(true), Some(true), false),
                LegacyOperationType::Rollback,
            ),
            (
                ("TON-TAC", Some("failed"), Some(true), Some(false), false),
                LegacyOperationType::TonTac,
            ),
            (
                ("TAC-TON", Some("success"), Some(true), Some(true), false),
                LegacyOperationType::TacTon,
            ),
            (
                ("UNKNOWN", None, Some(true), Some(false), false),
                LegacyOperationType::Unknown,
            ),
            (
                ("NEW-ROUTE", None, Some(true), Some(false), false),
                LegacyOperationType::ErrorType,
            ),
            (
                ("TON-TAC", Some("failed"), None, Some(true), true),
                LegacyOperationType::ErrorType,
            ),
        ];

        for ((route, status, finalized, rollback, insufficient_fee), expected) in cases {
            assert_eq!(
                project_v1_type(
                    Some(PROFILING_VERSION_V2),
                    Some(route),
                    status,
                    finalized,
                    rollback,
                    insufficient_fee
                ),
                expected
            );
        }
    }

    #[test]
    fn insufficient_fee_has_priority_over_other_failed_stage_reasons() {
        let data = v2_data(
            Some(OperationStatus::Failed),
            HashMap::from([
                (
                    StageType::ExecutedInTAC,
                    failed_stage(
                        r#"{"content":"execution failed","errorName":"ProxyCallError","internalBytesError":"","internalMsg":"ProxyCallError: default error"}"#,
                    ),
                ),
                (
                    StageType::ExecutedInTON,
                    failed_stage(
                        r#"{"content":"insufficient executor fee","errorName":"","internalBytesError":"","internalMsg":""}"#,
                    ),
                ),
            ]),
        );

        assert_eq!(
            derive_operation_error_reason(&data).as_deref(),
            Some(INSUFFICIENT_FEE_ERROR_REASON)
        );
    }

    #[test]
    fn error_reason_uses_latest_failed_stage_and_best_note_field() {
        let data = v2_data(
            Some(OperationStatus::Failed),
            HashMap::from([
                (
                    StageType::ExecutedInTAC,
                    failed_stage(r#"{"content":"earlier failure"}"#),
                ),
                (
                    StageType::ExecutedInTON,
                    failed_stage(
                        r#"{"content":"opaque payload","errorName":"ProxyCallError","internalMsg":"ProxyCallError: default error"}"#,
                    ),
                ),
            ]),
        );

        assert_eq!(
            derive_operation_error_reason(&data).as_deref(),
            Some("ProxyCallError: default error")
        );
    }

    #[test]
    fn error_reason_supports_content_and_plain_text_notes() {
        let json = v2_data(
            Some(OperationStatus::Failed),
            HashMap::from([(
                StageType::ExecutedInTAC,
                failed_stage(r#"{"content":"message expired"}"#),
            )]),
        );
        let plain = v2_data(
            Some(OperationStatus::Failed),
            HashMap::from([(StageType::ExecutedInTAC, failed_stage("plain failure"))]),
        );
        let successful = v2_data(
            Some(OperationStatus::Success),
            HashMap::from([(StageType::ExecutedInTAC, failed_stage("must stay hidden"))]),
        );

        assert_eq!(
            derive_operation_error_reason(&json).as_deref(),
            Some("message expired")
        );
        assert_eq!(
            derive_operation_error_reason(&plain).as_deref(),
            Some("plain failure")
        );
        assert_eq!(derive_operation_error_reason(&successful), None);
    }
}
