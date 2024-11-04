use super::TransformationStatus;
use verifier_alliance_entity::verified_contracts;

pub fn retrieve_code_transformation_status(
    id: Option<i64>,
    is_creation_code: bool,
    code_match: bool,
    code_values: Option<&serde_json::Value>,
) -> TransformationStatus {
    if code_match {
        if let Some(values) = code_values {
            if let Some(object) = values.as_object() {
                if object.contains_key("cborAuxdata") {
                    return TransformationStatus::WithAuxdata;
                } else {
                    return TransformationStatus::WithoutAuxdata;
                }
            } else {
                tracing::warn!(is_creation_code=is_creation_code,
                    verified_contract=?id,
                    "Transformation values is not an object")
            }
        } else {
            tracing::warn!(is_creation_code=is_creation_code,
                    verified_contract=?id,
                    "Was matched, but transformation values are null");
        }
    }

    TransformationStatus::NoMatch
}

pub fn calculate_max_status(
    deployment_verified_contracts: &[verified_contracts::Model],
    is_creation_code: bool,
) -> TransformationStatus {
    deployment_verified_contracts
        .iter()
        .map(|verified_contract| {
            let (does_match, values) = if is_creation_code {
                (
                    verified_contract.creation_match,
                    verified_contract.creation_values.as_ref(),
                )
            } else {
                (
                    verified_contract.runtime_match,
                    verified_contract.runtime_values.as_ref(),
                )
            };

            retrieve_code_transformation_status(
                Some(verified_contract.id),
                is_creation_code,
                does_match,
                values,
            )
        })
        .max()
        .unwrap_or(TransformationStatus::NoMatch)
}
