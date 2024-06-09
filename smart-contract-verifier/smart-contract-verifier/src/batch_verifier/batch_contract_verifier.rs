use crate::{
    batch_verifier::{
        artifacts::{cbor_auxdata, CodeArtifacts},
        compilation::{self, CompilationResult},
        errors::{BatchError, VerificationError, VerificationErrorKind},
        transformations,
    },
    compiler::CompilerInput,
    Compilers, Contract, DetailedVersion, MatchType, SolidityCompiler,
};
use std::collections::BTreeMap;

pub type VerificationResult = crate::batch_verifier::VerificationResult<BatchSuccess>;

#[derive(Clone, Debug)]
pub struct Match {
    pub match_type: MatchType,
    pub values: serde_json::Value,
    pub transformations: serde_json::Value,
}

#[derive(Clone, Debug, Default)]
pub struct BatchSuccess {
    pub compiler: String,
    pub compiler_version: String,
    pub language: String,
    pub compiler_settings: serde_json::Value,
    pub creation_code: Vec<u8>,
    pub runtime_code: Vec<u8>,
    pub file_name: String,
    pub contract_name: String,
    pub sources: BTreeMap<String, String>,
    pub compilation_artifacts: serde_json::Value,
    pub creation_code_artifacts: serde_json::Value,
    pub runtime_code_artifacts: serde_json::Value,
    pub creation_match: Option<Match>,
    pub runtime_match: Option<Match>,
}

pub async fn verify_solidity(
    compilers: &Compilers<SolidityCompiler>,
    compiler_version: DetailedVersion,
    contracts: Vec<Contract>,
    compiler_input: &foundry_compilers::CompilerInput,
) -> Result<Vec<VerificationResult>, BatchError> {
    let (raw_compiler_output, _) = compilers
        .compile(&compiler_version, compiler_input, None)
        .await?;

    let (modified_raw_compiler_output, _) = {
        let compiler_input = compiler_input.clone().modify();
        compilers
            .compile(&compiler_version, &compiler_input, None)
            .await?
    };

    let compilation_result = compilation::parse_solidity_contracts(
        compiler_version,
        compiler_input,
        raw_compiler_output,
        modified_raw_compiler_output,
    )
    .map_err(|err| {
        tracing::error!("parsing compiled contracts failed: {err:#}");
        BatchError::Internal(err)
    })?;

    let mut results = vec![];
    for contract in contracts {
        results.push(verify_contract(contract, &compilation_result)?);
    }

    Ok(results)
}

fn verify_contract(
    contract: Contract,
    compilation_result: &CompilationResult,
) -> Result<VerificationResult, BatchError> {
    let mut successes: Vec<BatchSuccess> = Vec::new();
    let mut failures: Vec<VerificationError> = Vec::new();
    for parsed_contract in &compilation_result.parsed_contracts {
        let convert_error = |mut kind, context: &'static str| {
            if let VerificationErrorKind::InternalError(err) = kind {
                kind = VerificationErrorKind::InternalError(err.context(context))
            }
            VerificationError::new(
                parsed_contract.file_name.clone(),
                parsed_contract.contract_name.clone(),
                kind,
            )
        };

        let (does_creation_match, creation_values, creation_transformations) =
            match &contract.creation_code {
                Some(contract_code) => {
                    match transformations::process_creation_code(
                        contract_code,
                        parsed_contract.creation_code.to_vec(),
                        &parsed_contract.compilation_artifacts,
                        CodeArtifacts::CreationCodeArtifacts(
                            parsed_contract.creation_code_artifacts.clone(),
                        ),
                    ) {
                        Ok((processed_code, values, transformations)) => {
                            (&processed_code == contract_code, values, transformations)
                        }
                        Err(err) => {
                            failures.push(convert_error(err, "process creation code"));
                            continue;
                        }
                    }
                }
                None => (false, Default::default(), Default::default()),
            };

        let (does_runtime_match, runtime_values, runtime_transformations) =
            match &contract.runtime_code {
                Some(contract_code) => {
                    match transformations::process_runtime_code(
                        contract_code,
                        parsed_contract.runtime_code.to_vec(),
                        &parsed_contract.compilation_artifacts,
                        CodeArtifacts::RuntimeCodeArtifacts(
                            parsed_contract.runtime_code_artifacts.clone(),
                        ),
                    ) {
                        Ok((processed_code, values, transformations)) => {
                            (&processed_code == contract_code, values, transformations)
                        }
                        Err(err) => {
                            failures.push(convert_error(err, "process runtime code"));
                            continue;
                        }
                    }
                }
                None => (false, Default::default(), Default::default()),
            };

        if !does_creation_match && !does_runtime_match {
            failures.push(VerificationError::new(
                parsed_contract.file_name.clone(),
                parsed_contract.contract_name.clone(),
                VerificationErrorKind::CodeMismatch,
            ));

            continue;
        }

        let success = BatchSuccess {
            creation_code: parsed_contract.creation_code.to_vec(),
            runtime_code: parsed_contract.runtime_code.to_vec(),
            compiler: compilation_result.compiler.clone(),
            compiler_version: compilation_result.compiler_version.clone(),
            language: compilation_result.language.clone(),
            file_name: parsed_contract.file_name.clone(),
            contract_name: parsed_contract.contract_name.clone(),
            sources: compilation_result.sources.clone(),
            compiler_settings: compilation_result.compiler_settings.clone(),
            compilation_artifacts: serde_json::to_value(
                parsed_contract.compilation_artifacts.clone(),
            )
            .expect("is json serializable"),
            creation_code_artifacts: serde_json::to_value(
                parsed_contract.creation_code_artifacts.clone(),
            )
            .expect("is json serializable"),
            runtime_code_artifacts: serde_json::to_value(
                parsed_contract.runtime_code_artifacts.clone(),
            )
            .expect("is json serializable"),
            creation_match: does_creation_match.then_some(Match {
                match_type: match_type(
                    creation_values.clone(),
                    &parsed_contract.creation_code_artifacts.cbor_auxdata,
                ),
                values: creation_values,
                transformations: creation_transformations,
            }),
            runtime_match: does_runtime_match.then_some(Match {
                match_type: match_type(
                    runtime_values.clone(),
                    &parsed_contract.runtime_code_artifacts.cbor_auxdata,
                ),
                values: runtime_values,
                transformations: runtime_transformations,
            }),
        };

        successes.push(success);
    }

    match choose_best_contract(successes) {
        Some(success) => Ok(VerificationResult::Success(success)),
        None => Ok(VerificationResult::Failure(failures)),
    }
}

fn choose_best_contract(successes: Vec<BatchSuccess>) -> Option<BatchSuccess> {
    if successes.is_empty() {
        return None;
    }

    let mut best_contract = BatchSuccess::default();
    for success in successes {
        if best_contract.creation_match.is_some() && best_contract.runtime_match.is_some() {
            return Some(best_contract);
        }

        if success.creation_match.is_some() && success.runtime_match.is_some() {
            best_contract = success;
            continue;
        }

        if success.creation_match.is_some() && best_contract.creation_match.is_none() {
            best_contract = success;
            continue;
        }

        if success.runtime_match.is_some()
            && best_contract.creation_match.is_none()
            && best_contract.runtime_match.is_none()
        {
            best_contract = success;
            continue;
        }
    }

    Some(best_contract)
}

fn match_type(values: serde_json::Value, cbor_auxdata: &cbor_auxdata::CborAuxdata) -> MatchType {
    // if no cbor_auxdata is present, no metadata hash exists to check on exact matches
    if cbor_auxdata.is_empty() {
        return MatchType::Partial;
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Values {
        #[serde(default)]
        cbor_auxdata: BTreeMap<String, String>,
    }

    let values: Values = serde_json::from_value(values).unwrap();

    if values.cbor_auxdata.is_empty() {
        MatchType::Full
    } else {
        MatchType::Partial
    }
}
