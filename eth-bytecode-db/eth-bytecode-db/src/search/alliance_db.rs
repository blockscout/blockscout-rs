use super::MatchContract;
use crate::{verification::MatchType, ToHex};
use anyhow::Context;
use sea_orm::DatabaseConnection;
use std::collections::BTreeMap;
use verification_common::solidity_libraries;
use verifier_alliance_database::VerifiedContractMatches;

pub async fn find_contract(
    db: &DatabaseConnection,
    chain_id: i64,
    contract_address: Vec<u8>,
) -> Result<Vec<MatchContract>, anyhow::Error> {
    let retrieved_values =
        verifier_alliance_database::find_verified_contracts(db, chain_id, contract_address)
            .await
            .context("retrieving verified contracts")?;

    let matches = retrieved_values
        .into_iter()
        .map(match_from_verified_contract)
        .collect::<Result<Vec<_>, _>>()
        .context("convert verified contracts into matches")?;

    if matches
        .iter()
        .any(|contract| contract.match_type == MatchType::Full)
    {
        Ok(matches
            .into_iter()
            .filter(|contract| contract.match_type == MatchType::Full)
            .collect())
    } else {
        Ok(matches)
    }
}

fn match_from_verified_contract(
    retrieved_value: verifier_alliance_database::RetrievedVerifiedContract,
) -> Result<MatchContract, anyhow::Error> {
    let updated_at = retrieved_value.updated_at.naive_utc();

    let verified_contract = retrieved_value.verified_contract;
    let compiled_contract = verified_contract.compiled_contract;

    let abi = compiled_contract.compilation_artifacts.abi.clone();
    let compilation_artifacts: serde_json::Value = compiled_contract.compilation_artifacts.into();
    let creation_code_artifacts: serde_json::Value =
        compiled_contract.creation_code_artifacts.into();
    let runtime_code_artifacts: serde_json::Value = compiled_contract.runtime_code_artifacts.into();
    let libraries = extract_libraries(
        &compiled_contract.compiler_settings,
        &verified_contract.matches,
    )?;
    let match_contract = MatchContract {
        updated_at,
        file_name: extract_file_name(&compiled_contract.fully_qualified_name)?,
        contract_name: compiled_contract.name,
        compiler_version: compiled_contract.version,
        compiler_settings: compiled_contract.compiler_settings,
        source_type: compiled_contract.language.into(),
        source_files: compiled_contract.sources,
        abi: abi.as_ref().map(|value| value.to_string()),
        constructor_arguments: extract_constructor_arguments(&verified_contract.matches),
        match_type: extract_match_type(&verified_contract.matches),
        compilation_artifacts: Some(compilation_artifacts.to_string()),
        creation_input_artifacts: Some(creation_code_artifacts.to_string()),
        deployed_bytecode_artifacts: Some(runtime_code_artifacts.to_string()),
        raw_creation_input: compiled_contract.creation_code,
        raw_deployed_bytecode: compiled_contract.runtime_code,
        is_blueprint: false,
        libraries,
    };

    Ok(match_contract)
}

fn extract_file_name(fully_qualified_name: &str) -> Result<String, anyhow::Error> {
    let file_name_parts = fully_qualified_name.split(':').collect::<Vec<_>>();
    if file_name_parts.len() < 2 {
        anyhow::bail!(
                "the contract has invalid fully_qualified_name: at least one ':' symbol should exist: {}",
                fully_qualified_name
            )
    }
    // We discard the last element, as it should be a contract name
    Ok(file_name_parts[..file_name_parts.len() - 1].join(":"))
}

fn extract_constructor_arguments(match_values: &VerifiedContractMatches) -> Option<String> {
    let match_value = match match_values {
        VerifiedContractMatches::OnlyCreation { creation_match } => creation_match,
        VerifiedContractMatches::OnlyRuntime { .. } => return None,
        VerifiedContractMatches::Complete { creation_match, .. } => creation_match,
    };

    match_value
        .values
        .constructor_arguments
        .as_ref()
        .map(|value| value.to_hex())
}

fn extract_match_type(verified_contract_matches: &VerifiedContractMatches) -> MatchType {
    let full_match = match verified_contract_matches {
        VerifiedContractMatches::OnlyCreation { creation_match } => creation_match.metadata_match,
        VerifiedContractMatches::OnlyRuntime { runtime_match } => runtime_match.metadata_match,
        VerifiedContractMatches::Complete { creation_match, .. } => creation_match.metadata_match,
    };

    if full_match {
        MatchType::Full
    } else {
        MatchType::Partial
    }
}

fn extract_libraries(
    compiler_settings: &serde_json::Value,
    verified_contract_matches: &VerifiedContractMatches,
) -> Result<BTreeMap<String, String>, anyhow::Error> {
    let mut libraries = solidity_libraries::try_parse_compiler_linked_libraries(compiler_settings)?;
    match verified_contract_matches {
        VerifiedContractMatches::OnlyCreation {
            creation_match: match_,
        }
        | VerifiedContractMatches::OnlyRuntime {
            runtime_match: match_,
        } => {
            libraries.extend(solidity_libraries::parse_manually_linked_libraries(match_));
        }
        VerifiedContractMatches::Complete {
            creation_match,
            runtime_match,
        } => {
            libraries.extend(solidity_libraries::parse_manually_linked_libraries(
                creation_match,
            ));
            libraries.extend(solidity_libraries::parse_manually_linked_libraries(
                runtime_match,
            ));
        }
    }
    Ok(libraries)
}
