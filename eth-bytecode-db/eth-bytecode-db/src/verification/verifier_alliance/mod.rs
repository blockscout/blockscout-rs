mod parsers;
mod transformations;

/**********************************************/

use verifier_alliance_entity::contract_deployments;

pub struct CodeMatch {
    pub does_match: bool,
    pub values: Option<serde_json::Value>,
    pub transformations: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum TransformationStatus {
    NoMatch,
    WithAuxdata,
    WithoutAuxdata,
}

pub fn verify_creation_code(
    contract_deployment: &contract_deployments::Model,
    deployed_code: Option<Vec<u8>>,
    compiled_code: Vec<u8>,
    code_artifacts: Option<serde_json::Value>,
) -> Result<CodeMatch, anyhow::Error> {
    verify_code(
        contract_deployment,
        deployed_code,
        compiled_code,
        code_artifacts,
        transformations::process_creation_code,
    )
}

pub fn verify_runtime_code(
    contract_deployment: &contract_deployments::Model,
    deployed_code: Option<Vec<u8>>,
    compiled_code: Vec<u8>,
    code_artifacts: Option<serde_json::Value>,
) -> Result<CodeMatch, anyhow::Error> {
    verify_code(
        contract_deployment,
        deployed_code,
        compiled_code,
        code_artifacts,
        transformations::process_runtime_code,
    )
}

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

fn verify_code<F>(
    contract_deployment: &contract_deployments::Model,
    deployed_code: Option<Vec<u8>>,
    compiled_code: Vec<u8>,
    code_artifacts: Option<serde_json::Value>,
    transformations: F,
) -> Result<CodeMatch, anyhow::Error>
where
    F: Fn(
        &[u8],
        Vec<u8>,
        serde_json::Value,
    ) -> Result<(Vec<u8>, serde_json::Value, serde_json::Value), anyhow::Error>,
{
    let code_artifacts = code_artifacts.ok_or(anyhow::anyhow!("code artifacts are missing"))?;
    let code_match_details = deployed_code.and_then(|deployed_code| {
        let result = process_code(
            &deployed_code,
            compiled_code,
            code_artifacts,
            transformations,
        );

        match result {
            Ok(res) => Some(res),
            Err(err) => {
                let contract_address = format!("0x{}", hex::encode(&contract_deployment.address));
                tracing::warn!(
                    contract_address = contract_address.to_string(),
                    chain_id = contract_deployment.chain_id.to_string(),
                    "code processing failed; err={err:#}"
                );
                None
            }
        }
    });

    let code_match = match code_match_details {
        None => CodeMatch {
            does_match: false,
            transformations: None,
            values: None,
        },
        Some((values, transformations)) => CodeMatch {
            does_match: true,
            transformations: Some(transformations),
            values: Some(values),
        },
    };
    Ok(code_match)
}

fn process_code<F>(
    deployed_code: &[u8],
    compiled_code: Vec<u8>,
    code_artifacts: serde_json::Value,
    processing_function: F,
) -> Result<(serde_json::Value, serde_json::Value), anyhow::Error>
where
    F: Fn(
        &[u8],
        Vec<u8>,
        serde_json::Value,
    ) -> Result<(Vec<u8>, serde_json::Value, serde_json::Value), anyhow::Error>,
{
    let (processed_code, values, transformations) =
        processing_function(deployed_code, compiled_code.clone(), code_artifacts)?;

    if processed_code != deployed_code {
        return Err(anyhow::anyhow!(
            "processed code does not match to the actually deployed one"
        ));
    }

    Ok((values, transformations))
}
