use blockscout_display_bytes::ToHex;
use serde_json::Value;
use smart_contract_verifier::{
    solidity,
    verify_new::{Error, VerificationResult},
    BatchError, BatchVerificationResult, DetailedVersion, Language,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::{
    v2 as proto,
    v2::{BatchVerifyResponse, CompilationFailure},
};
use std::{collections::BTreeMap, path::PathBuf, str::FromStr};
use tonic::{Response, Status};
use verification_common::verifier_alliance;

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

pub fn compilation_error_new(message: impl Into<String>) -> BatchVerifyResponse {
    BatchVerifyResponse {
        verification_result: Some(
            proto::batch_verify_response::VerificationResult::CompilationFailure(
                CompilationFailure {
                    message: message.into(),
                },
            ),
        ),
    }
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

pub fn process_error(error: Error) -> Result<BatchVerifyResponse, Status> {
    match error {
        err @ Error::CompilerNotFound(_) => Err(Status::invalid_argument(err.to_string())),
        err @ Error::Internal(_) => {
            let formatted_error = format!("{err:#?}");
            tracing::error!(err = formatted_error, "internal error");
            Err(Status::internal(formatted_error))
        }
        err @ Error::Compilation(_) => Ok(compilation_error_new(err.to_string())),
    }
}

pub fn process_verification_results_new(
    values: Vec<VerificationResult>,
) -> Result<BatchVerifyResponse, Status> {
    let items = values
        .into_iter()
        .map(process_verification_result_new)
        .collect::<Result<_, _>>()?;

    Ok(BatchVerifyResponse {
        verification_result: Some(
            proto::batch_verify_response::VerificationResult::ContractVerificationResults(
                proto::batch_verify_response::ContractVerificationResults { items },
            ),
        ),
    })
}

fn process_verification_result_new(
    value: VerificationResult,
) -> Result<proto::ContractVerificationResult, Status> {
    if value.is_empty() {
        let verification_result = proto::contract_verification_result::VerificationResult::Failure(
            proto::ContractVerificationFailure {},
        );
        return Ok(proto::ContractVerificationResult {
            verification_result: Some(verification_result),
        });
    }

    let verifying_contract = value.into_iter().next().unwrap();

    let verification_result = proto::contract_verification_result::VerificationResult::Success(
        proto::ContractVerificationSuccess {
            creation_code: verifying_contract.creation_code.to_hex(),
            runtime_code: verifying_contract.runtime_code.to_hex(),
            compiler: proto_compiler_from_language(verifying_contract.language).into(),
            compiler_version: verifying_contract.compiler_version,
            language: proto_language_from_language(verifying_contract.language).into(),
            file_name: verifying_contract.fully_qualified_name.file_name(),
            contract_name: verifying_contract.fully_qualified_name.contract_name(),
            sources: verifying_contract.sources,
            compiler_settings: verifying_contract.compiler_settings.to_string(),
            compilation_artifacts: Value::from(verifying_contract.compilation_artifacts)
                .to_string(),
            creation_code_artifacts: Value::from(verifying_contract.creation_code_artifacts)
                .to_string(),
            runtime_code_artifacts: Value::from(verifying_contract.runtime_code_artifacts)
                .to_string(),
            creation_match_details: parse_maybe_match(verifying_contract.creation_match),
            runtime_match_details: parse_maybe_match(verifying_contract.runtime_match),
        },
    );

    Ok(proto::ContractVerificationResult {
        verification_result: Some(verification_result),
    })
}

fn proto_compiler_from_language(
    language: Language,
) -> proto::contract_verification_success::compiler::Compiler {
    match language {
        Language::Solidity | Language::Yul => {
            proto::contract_verification_success::compiler::Compiler::Solc
        }
        Language::Vyper => proto::contract_verification_success::compiler::Compiler::Vyper,
    }
}

fn proto_language_from_language(
    language: Language,
) -> proto::contract_verification_success::language::Language {
    match language {
        Language::Solidity => proto::contract_verification_success::language::Language::Solidity,
        Language::Yul => proto::contract_verification_success::language::Language::Yul,
        Language::Vyper => proto::contract_verification_success::language::Language::Vyper,
    }
}

fn parse_maybe_match(
    value: Option<verifier_alliance::Match>,
) -> Option<proto::contract_verification_success::MatchDetails> {
    if let Some(value) = value {
        let match_type = if value.metadata_match {
            proto::contract_verification_success::MatchType::Full
        } else {
            proto::contract_verification_success::MatchType::Partial
        };

        return Some(proto::contract_verification_success::MatchDetails {
            match_type: match_type.into(),
            values: Value::from(value.values).to_string(),
            transformations: Value::from(value.transformations).to_string(),
        });
    }
    None
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
