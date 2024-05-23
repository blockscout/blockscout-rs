use super::MatchContract;
use crate::verification::{MatchType, SourceType};
use anyhow::Context;
use sea_orm::{entity::prelude::Decimal, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use verifier_alliance_entity::{compiled_contracts, contract_deployments, verified_contracts};

pub async fn find_contract<C: ConnectionTrait>(
    db: &C,
    chain_id: i64,
    contract_address: Vec<u8>,
) -> Result<Vec<MatchContract>, anyhow::Error> {
    let compiled_verified_pairs =
        retrieve_compiled_verified_pairs(db, chain_id, contract_address).await?;

    let matches = compiled_verified_pairs
        .into_iter()
        .map(|(compiled_contract, verified_contract)| {
            match_contract_from_model(compiled_contract, verified_contract)
        })
        .collect::<Result<Vec<_>, _>>()
        .context("convert compiled and verified contracts into matches")?;

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

async fn retrieve_compiled_verified_pairs<C: ConnectionTrait>(
    db: &C,
    chain_id: i64,
    contract_address: Vec<u8>,
) -> Result<Vec<(compiled_contracts::Model, verified_contracts::Model)>, anyhow::Error> {
    let contract_deployment = if let Some(model) = contract_deployments::Entity::find()
        .filter(contract_deployments::Column::ChainId.eq(Decimal::from(chain_id)))
        .filter(contract_deployments::Column::Address.eq(contract_address))
        .one(db)
        .await
        .context("retrieve contract deployment")?
    {
        model
    } else {
        return Ok(vec![]);
    };

    let verified_contracts = verified_contracts::Entity::find()
        .filter(verified_contracts::Column::DeploymentId.eq(contract_deployment.id))
        .all(db)
        .await
        .context("retrieve verified contracts")?;

    let mut compiled_verified_pairs = vec![];
    for verified_contract in verified_contracts {
        let compiled_contract = compiled_contracts::Entity::find_by_id(
            verified_contract.compilation_id,
        )
        .one(db)
        .await
        .context(format!(
            "retrieve compiled contract for {}",
            hex::encode(verified_contract.compilation_id)
        ))?
        .expect(
            "Compilation must exist for the given verified contract due to foreign key constraint",
        );
        compiled_verified_pairs.push((compiled_contract, verified_contract))
    }

    Ok(compiled_verified_pairs)
}

fn match_contract_from_model(
    compiled_contract: compiled_contracts::Model,
    verified_contract: verified_contracts::Model,
) -> Result<MatchContract, anyhow::Error> {
    let updated_at = compiled_contract
        .updated_at
        .naive_utc()
        .max(verified_contract.updated_at.naive_utc());

    let match_type = extract_match_type(&compiled_contract, &verified_contract)?;

    let source_files = serde_json::from_value(compiled_contract.sources)
        .context("compiled contract sources are not valid BTreeMap<String, String>")?;

    let match_contract = MatchContract {
        updated_at,
        file_name: extract_file_name(&compiled_contract.fully_qualified_name)?,
        contract_name: compiled_contract.name,
        compiler_version: compiled_contract.version,
        compiler_settings: compiled_contract.compiler_settings.to_string(),
        source_type: extract_source_type(&compiled_contract.language)?,
        source_files,
        abi: extract_abi(&compiled_contract.compilation_artifacts)?,
        constructor_arguments: extract_constructor_arguments(&verified_contract)?,
        match_type,
        compilation_artifacts: Some(compiled_contract.compilation_artifacts.to_string()),
        creation_input_artifacts: Some(compiled_contract.creation_code_artifacts.to_string()),
        deployed_bytecode_artifacts: Some(compiled_contract.runtime_code_artifacts.to_string()),

        // They are not used in the final Source returned to the user,
        // so for complexity reasons, we just ignore them for now
        raw_creation_input: vec![],
        raw_deployed_bytecode: vec![],
        is_blueprint: false,
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

fn extract_source_type(language: &str) -> Result<SourceType, anyhow::Error> {
    Ok(match language {
        "solidity" => SourceType::Solidity,
        "yul" => SourceType::Yul,
        "vyper" => SourceType::Vyper,
        _ => anyhow::bail!(
            "the contract has invalid compiler language; \
            expected one of 'solidity', 'yul', 'vyper'; found: {}",
            language,
        ),
    })
}

fn extract_abi(compilation_artifacts: &serde_json::Value) -> Result<Option<String>, anyhow::Error> {
    Ok(compilation_artifacts
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("'compilation_artifacts' is not an object"))?
        .get("abi")
        .map(|value| value.to_string()))
}

fn extract_constructor_arguments(
    verified_contract: &verified_contracts::Model,
) -> Result<Option<String>, anyhow::Error> {
    if verified_contract.creation_match {
        let values = verified_contract
            .creation_values
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!("'creation_values' does not exist, though 'creation_match' is true")
            })?
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("'creation_values' is not an object"))?;
        if let Some(arguments) = values.get("constructorArguments") {
            return Ok(Some(
                arguments
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("'constructor_arguments' value is not a string"))
                    .map(ToString::to_string)?,
            ));
        }
    }

    Ok(None)
}

fn extract_match_type(
    compiled_contract: &compiled_contracts::Model,
    verified_contract: &verified_contracts::Model,
) -> Result<MatchType, anyhow::Error> {
    // If creation code matches, we are considering it with greater priority,
    // assuming that creation code full match automatically implies runtime code full match
    if verified_contract.creation_match {
        let code_artifacts = compiled_contract
            .creation_code_artifacts
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("'creation_code_artifacts' is not an object"))?;
        let values = verified_contract
            .creation_values
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!("'creation_values' does not exist, though 'creation_match' is true")
            })?
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("'creation_values' is not an object"))?;
        return Ok(
            match (code_artifacts.get("cborAuxdata"), values.get("cborAuxdata")) {
                (Some(_), None) => MatchType::Full,
                _ => MatchType::Partial,
            },
        );
    }

    if verified_contract.runtime_match {
        let code_artifacts = compiled_contract
            .runtime_code_artifacts
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("'runtime_code_artifacts' is not an object"))?;
        let values = verified_contract
            .runtime_values
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!("'runtime_values' does not exist, though 'runtime_match' is true")
            })?
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("'runtime_values' is not an object"))?;
        return Ok(
            match (code_artifacts.get("cborAuxdata"), values.get("cborAuxdata")) {
                (Some(_), None) => MatchType::Full,
                _ => MatchType::Partial,
            },
        );
    }

    Err(anyhow::anyhow!(
        "neither creation nor runtime codes have a match"
    ))
}
