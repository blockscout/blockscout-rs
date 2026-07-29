// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::client::models::profiling::{LegacyOperationType, OperationRoute, Stage};
use std::{collections::HashMap, str::FromStr};

pub const PROFILING_VERSION_V1: i16 = 1;
pub const PROFILING_VERSION_V2: i16 = 2;

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

pub fn project_v1_type(
    profiling_version: i16,
    stored_type: Option<&str>,
    op_status: Option<&str>,
    finalized: Option<bool>,
    rollback: Option<bool>,
    has_insufficient_fee: bool,
) -> LegacyOperationType {
    if profiling_version != PROFILING_VERSION_V2 {
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

    #[test]
    fn v2_projection_ignores_technical_status() {
        assert_eq!(
            project_v1_type(2, Some("TON-TAC"), None, Some(false), Some(false), false),
            LegacyOperationType::Pending
        );
    }

    #[test]
    fn failed_is_required_for_special_v2_projection() {
        assert_eq!(
            project_v1_type(
                2,
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
                2,
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
                2,
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
            project_v1_type(1, Some("INSUFFICIENT-FEE"), None, None, None, false),
            LegacyOperationType::InsufficientFee
        );
    }
}
