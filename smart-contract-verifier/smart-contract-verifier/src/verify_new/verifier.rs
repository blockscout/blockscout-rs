use super::{
    compilation,
    compilation::CompilationResult,
    evm_compilers::{EvmCompiler, EvmCompilersPool},
    verification,
    verification::OnChainCode,
    Error,
};
use crate::{DetailedVersion, FullyQualifiedName, Language};
use std::collections::BTreeMap;
use verification_common::verifier_alliance::{
    CompilationArtifacts, CreationCodeArtifacts, Match, RuntimeCodeArtifacts,
};

/// The contract to be verified.
// may be extended with contract metadata (address and chain_id) later
pub struct OnChainContract {
    pub on_chain_code: OnChainCode,
}

pub struct VerifyingContract {
    pub fully_qualified_name: FullyQualifiedName,
    pub language: Language,
    pub compiler_version: String,
    pub compiler_settings: serde_json::Value,
    pub sources: BTreeMap<String, String>,
    pub creation_code: Vec<u8>,
    pub runtime_code: Vec<u8>,
    pub compilation_artifacts: CompilationArtifacts,
    pub creation_code_artifacts: CreationCodeArtifacts,
    pub runtime_code_artifacts: RuntimeCodeArtifacts,
    pub runtime_match: Option<Match>,
    pub creation_match: Option<Match>,
}

pub type VerificationResult = Vec<VerifyingContract>;

pub async fn compile_and_verify<C: EvmCompiler>(
    to_verify: Vec<OnChainContract>,
    compilers: &EvmCompilersPool<C>,
    compiler_version: &DetailedVersion,
    compiler_input: C::CompilerInput,
) -> Result<Vec<VerificationResult>, Error> {
    let compilation_result =
        compilation::compile(compilers, compiler_version, compiler_input).await?;

    let mut verification_results = vec![];
    for contract in to_verify {
        verification_results.push(verify_on_chain_contract(contract, &compilation_result)?);
    }

    Ok(verification_results)
}

fn verify_on_chain_contract(
    contract: OnChainContract,
    compilation_result: &CompilationResult,
) -> Result<VerificationResult, Error> {
    let mut successes = vec![];
    for (fully_qualified_name, contract_artifacts) in &compilation_result.artifacts {
        let verify_contract_result = verification::verify_contract(
            contract.on_chain_code.clone(),
            contract_artifacts.code.clone(),
            contract_artifacts.compilation_artifacts.clone(),
            contract_artifacts.creation_code_artifacts.clone(),
            contract_artifacts.runtime_code_artifacts.clone(),
        );
        let maybe_verifying_contract = process_verify_contract_result(
            compilation_result,
            fully_qualified_name,
            &verify_contract_result,
        )?;

        if let Some(verifying_contract) = maybe_verifying_contract {
            successes.push(verifying_contract);
        }
    }

    Ok(successes)
}

fn process_verify_contract_result(
    compilation_result: &CompilationResult,
    contract_fully_qualified_name: &FullyQualifiedName,
    verify_contract_result: &verification::VerificationResult,
) -> Result<Option<VerifyingContract>, Error> {
    let (runtime_match, creation_match) = match verify_contract_result.clone() {
        verification::VerificationResult::Failure => return Ok(None),
        verification::VerificationResult::RuntimeMatch { runtime_match } => {
            (Some(runtime_match), None)
        }
        verification::VerificationResult::CreationMatch { creation_match } => {
            (None, Some(creation_match))
        }
        verification::VerificationResult::CompleteMatch {
            runtime_match,
            creation_match,
        } => (Some(runtime_match), Some(creation_match)),
    };

    let contract_compilation_artifacts = compilation_result
        .artifacts
        .get(contract_fully_qualified_name)
        .expect("key was obtained by iterating through `compilation_result.artifacts`");

    let verifying_contract = VerifyingContract {
        fully_qualified_name: contract_fully_qualified_name.clone(),
        language: compilation_result.language,
        compiler_version: compilation_result.compiler_version.clone(),
        compiler_settings: compilation_result.compiler_settings.clone(),
        sources: compilation_result.sources.clone(),
        creation_code: contract_compilation_artifacts.code.creation.clone(),
        runtime_code: contract_compilation_artifacts.code.runtime.clone(),
        compilation_artifacts: contract_compilation_artifacts.compilation_artifacts.clone(),
        creation_code_artifacts: contract_compilation_artifacts
            .creation_code_artifacts
            .clone(),
        runtime_code_artifacts: contract_compilation_artifacts
            .runtime_code_artifacts
            .clone(),
        runtime_match,
        creation_match,
    };

    Ok(Some(verifying_contract))
}
