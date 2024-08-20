use smart_contract_verifier::{solidity, BatchError, BatchVerificationResult, DetailedVersion};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::{
    v2 as proto,
    v2::{BatchVerifyResponse, CompilationFailure},
};
use std::{collections::BTreeMap, path::PathBuf, str::FromStr};
use tonic::{Response, Status};

pub fn from_proto_contracts_to_inner(
    proto: &[proto::Contract],
) -> Result<Vec<smart_contract_verifier::Contract>, Status> {
    let parse_code =
        |code: &Option<String>| code.as_ref().map(|code| super::from_hex(code)).transpose();
    let mut inner_contracts = Vec::new();
    for proto_contract in proto {
        let inner_contract = smart_contract_verifier::Contract {
            creation_code: parse_code(&proto_contract.creation_code).map_err(|err| {
                Status::invalid_argument(format!("Invalid creation code: {:?}", err))
            })?,
            runtime_code: parse_code(&proto_contract.runtime_code).map_err(|err| {
                Status::invalid_argument(format!("Invalid runtime code: {:?}", err))
            })?,
        };
        inner_contracts.push(inner_contract);
    }
    Ok(inner_contracts)
}

pub fn from_proto_compiler_version_to_inner(proto: &str) -> Result<DetailedVersion, Status> {
    DetailedVersion::from_str(proto)
        .map_err(|err| Status::invalid_argument(format!("Invalid compiler version: {}", err)))
}

pub fn from_proto_solidity_multi_part_content_to_inner(
    sources: BTreeMap<String, String>,
    evm_version: Option<String>,
    optimization_runs: Option<u32>,
    libraries: BTreeMap<String, String>,
) -> Result<solidity::multi_part::MultiFileContent, Status> {
    let sources = sources
        .into_iter()
        .map(|(file, content)| (PathBuf::from(file), content))
        .collect();

    let evm_version = evm_version
        .as_ref()
        .map(|value| foundry_compilers::EvmVersion::from_str(value))
        .transpose()
        .map_err(|err| Status::invalid_argument(format!("Invalid evm version: {}", err)))?;

    Ok(solidity::multi_part::MultiFileContent {
        sources,
        evm_version,
        optimization_runs: optimization_runs.map(|value| value as usize),
        contract_libraries: (!libraries.is_empty()).then_some(libraries),
    })
}

pub fn from_inner_match_type_to_proto(
    inner: smart_contract_verifier::MatchType,
) -> proto::contract_verification_success::MatchType {
    match inner {
        smart_contract_verifier::MatchType::Partial => {
            proto::contract_verification_success::MatchType::Partial
        }
        smart_contract_verifier::MatchType::Full => {
            proto::contract_verification_success::MatchType::Full
        }
    }
}

pub fn compilation_error(message: impl Into<String>) -> Response<BatchVerifyResponse> {
    Response::new(BatchVerifyResponse {
        verification_result: Some(
            proto::batch_verify_response::VerificationResult::CompilationFailure(
                CompilationFailure {
                    message: message.into(),
                },
            ),
        ),
    })
}

pub fn process_batch_error(error: BatchError) -> Result<Response<BatchVerifyResponse>, Status> {
    match error {
        BatchError::VersionNotFound(_) => Err(Status::invalid_argument(error.to_string())),
        BatchError::Compilation(_) => {
            let response = compilation_error(error.to_string());
            Ok(response)
        }
        BatchError::Internal(_) => Err(Status::internal(error.to_string())),
    }
}

pub fn process_verification_results(
    results: Vec<BatchVerificationResult>,
) -> Result<Response<BatchVerifyResponse>, Status> {
    let items = results
        .into_iter()
        .map(process_verification_result)
        .collect::<Result<Vec<_>, _>>()?;
    let response = BatchVerifyResponse {
        verification_result: Some(
            proto::batch_verify_response::VerificationResult::ContractVerificationResults(
                proto::batch_verify_response::ContractVerificationResults { items },
            ),
        ),
    };
    Ok(Response::new(response))
}

fn process_verification_result(
    result: BatchVerificationResult,
) -> Result<proto::ContractVerificationResult, Status> {
    let verification_result = match result {
        BatchVerificationResult::Failure(_) => {
            proto::contract_verification_result::VerificationResult::Failure(
                proto::ContractVerificationFailure {},
            )
        }
        BatchVerificationResult::Success(success) => {
            let compiler = proto::contract_verification_success::compiler::Compiler::from_str_name(
                &success.compiler.to_uppercase(),
            )
            .ok_or_else(|| Status::internal("invalid compiler returned internally"))?;
            let language = proto::contract_verification_success::language::Language::from_str_name(
                &success.language.to_uppercase(),
            )
            .ok_or_else(|| Status::internal("invalid language returned internally"))?;

            proto::contract_verification_result::VerificationResult::Success(
                proto::ContractVerificationSuccess {
                    creation_code: super::to_hex(success.creation_code),
                    runtime_code: super::to_hex(success.runtime_code),
                    compiler: compiler.into(),
                    compiler_version: success.compiler_version,
                    language: language.into(),
                    file_name: success.file_name,
                    contract_name: success.contract_name,
                    sources: success.sources,
                    compiler_settings: success.compiler_settings.to_string(),
                    compilation_artifacts: success.compilation_artifacts.to_string(),
                    creation_code_artifacts: success.creation_code_artifacts.to_string(),
                    runtime_code_artifacts: success.runtime_code_artifacts.to_string(),
                    creation_match_details: success.creation_match.as_ref().map(|creation_match| {
                        proto::contract_verification_success::MatchDetails {
                            match_type: from_inner_match_type_to_proto(creation_match.match_type)
                                .into(),
                            values: creation_match.values.to_string(),
                            transformations: creation_match.transformations.to_string(),
                        }
                    }),
                    runtime_match_details: success.runtime_match.as_ref().map(|runtime_match| {
                        proto::contract_verification_success::MatchDetails {
                            match_type: from_inner_match_type_to_proto(runtime_match.match_type)
                                .into(),
                            values: runtime_match.values.to_string(),
                            transformations: runtime_match.transformations.to_string(),
                        }
                    }),
                },
            )
        }
    };
    Ok(proto::ContractVerificationResult {
        verification_result: Some(verification_result),
    })
}
