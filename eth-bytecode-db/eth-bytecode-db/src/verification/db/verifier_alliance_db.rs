use super::{
    super::{types, verifier_alliance, SourceType},
    insert_then_select,
};
use anyhow::Context;
use blockscout_display_bytes::Bytes as DisplayBytes;
use sea_orm::{
    entity::prelude::ColumnTrait, ActiveValue::Set, DatabaseConnection, DatabaseTransaction,
    EntityTrait, JoinType, QueryFilter, QuerySelect, RelationTrait, TransactionTrait,
};
use verifier_alliance_entity::{
    code, compiled_contracts, contract_deployments, contracts, verified_contracts,
};

pub(crate) struct ContractDeploymentData {
    pub chain_id: i64,
    pub contract_address: Vec<u8>,
    pub transaction_hash: Vec<u8>,
}

pub(crate) async fn insert_data(
    db_client: &DatabaseConnection,
    source_response: types::DatabaseReadySource,
    deployment_data: ContractDeploymentData,
) -> Result<(), anyhow::Error> {
    let txn = db_client
        .begin()
        .await
        .context("begin database transaction")?;

    let contract = retrieve_contract(&txn, &deployment_data)
        .await
        .context("retrieve contract")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "contract was not found: chain_id={}, address={}, transaction_hash={}",
                deployment_data.chain_id,
                DisplayBytes::from(deployment_data.contract_address.clone()),
                DisplayBytes::from(deployment_data.transaction_hash.clone())
            )
        })?;

    let compiled_contract = insert_compiled_contract(&txn, source_response)
        .await
        .context("insert compiled_contract")?;

    let _verified_contract = insert_verified_contract(&txn, &contract, &compiled_contract)
        .await
        .context("insert verified_contract")?;

    txn.commit().await.context("commit transaction")?;

    Ok(())
}

async fn retrieve_contract(
    txn: &DatabaseTransaction,
    deployment_data: &ContractDeploymentData,
) -> Result<Option<contracts::Model>, anyhow::Error> {
    contracts::Entity::find()
        .join(
            JoinType::Join,
            contracts::Relation::ContractDeployments.def(),
        )
        .filter(contract_deployments::Column::ChainId.eq(deployment_data.chain_id))
        .filter(contract_deployments::Column::Address.eq(deployment_data.contract_address.clone()))
        .filter(
            contract_deployments::Column::TransactionHash
                .eq(deployment_data.transaction_hash.clone()),
        )
        .one(txn)
        .await
        .context("select from \"contracts\" joined with \"contract_deployments\"")
}

async fn retrieve_code(
    txn: &DatabaseTransaction,
    code_hash: Vec<u8>,
) -> Result<Option<code::Model>, anyhow::Error> {
    code::Entity::find_by_id(code_hash)
        .one(txn)
        .await
        .context("select from \"code\"")
}

async fn check_code_match<F>(
    txn: &DatabaseTransaction,
    deployed_code_hash: Vec<u8>,
    compiled_code_hash: Vec<u8>,
    code_artifacts: serde_json::Value,
    processing_function: F,
) -> Result<(bool, Option<serde_json::Value>, Option<serde_json::Value>), anyhow::Error>
where
    F: Fn(
        &[u8],
        Vec<u8>,
        serde_json::Value,
    ) -> Result<(serde_json::Value, serde_json::Value), anyhow::Error>,
{
    let deployed_code = retrieve_code(txn, deployed_code_hash)
        .await
        .context("retrieve deployed code")?
        .expect(
            "\"contracts\".\"code_hash\" has a foreign key constraint on \"code\".\"code_hash\"",
        );
    let compiled_code = retrieve_code(txn, compiled_code_hash).await.context("retrieve compiled code")?
        .expect("\"compiled_contracts\".\"code_hash\" has a foreign key constraint on \"code\".\"code_hash\"");

    let code_match_details = match (deployed_code.code, compiled_code.code) {
        (Some(deployed_code), Some(compiled_code)) => {
            match processing_function(&deployed_code, compiled_code, code_artifacts) {
                Ok(res) => Some(res),
                Err(err) => {
                    tracing::warn!("code processing failed; err={err:#}");
                    None
                }
            }
        }
        _ => None,
    };

    let (creation_match, creation_values, creation_transformations) = match code_match_details {
        None => (false, None, None),
        Some((values, transformations)) => (true, Some(values), Some(transformations)),
    };

    Ok((creation_match, creation_values, creation_transformations))
}

async fn insert_verified_contract(
    txn: &DatabaseTransaction,
    contract: &contracts::Model,
    compiled_contract: &compiled_contracts::Model,
) -> Result<verified_contracts::Model, anyhow::Error> {
    let (creation_match, creation_values, creation_transformations) = check_code_match(
        txn,
        contract.creation_code_hash.clone(),
        compiled_contract.creation_code_hash.clone(),
        compiled_contract.creation_code_artifacts.clone(),
        verifier_alliance::process_creation_code,
    )
    .await
    .context("check creation code match")?;
    let (runtime_match, runtime_values, runtime_transformations) = check_code_match(
        txn,
        contract.runtime_code_hash.clone(),
        compiled_contract.runtime_code_hash.clone(),
        compiled_contract.runtime_code_artifacts.clone(),
        verifier_alliance::process_runtime_code,
    )
    .await
    .context("check runtime code match")?;

    if !(creation_match || runtime_match) {
        return Err(anyhow::anyhow!(
            "neither creation code nor runtime code have not matched"
        ));
    };

    let active_model = verified_contracts::ActiveModel {
        id: Default::default(),
        compilation_id: Set(compiled_contract.id),
        contract_id: Set(contract.id),
        creation_match: Set(creation_match),
        creation_values: Set(creation_values),
        creation_transformations: Set(creation_transformations),
        runtime_match: Set(runtime_match),
        runtime_values: Set(runtime_values),
        runtime_transformations: Set(runtime_transformations),
    };

    let (verified_contract, _inserted) = insert_then_select!(
        txn,
        verified_contracts,
        active_model,
        [
            (CompilationId, compiled_contract.id),
            (ContractId, contract.id),
        ]
    )?;

    Ok(verified_contract)
}

async fn insert_compiled_contract(
    txn: &DatabaseTransaction,
    source: types::DatabaseReadySource,
) -> Result<compiled_contracts::Model, anyhow::Error> {
    let (compiler, language) = match source.source_type {
        SourceType::Solidity => ("solc", "solidity"),
        SourceType::Vyper => ("vyper", "vyper"),
        SourceType::Yul => ("solc", "yul"),
    };
    let fully_qualified_name = format!("{}:{}", source.file_name, source.contract_name);
    let sources = serde_json::to_value(source.source_files)
        .context("serializing source files to json value")?;
    let compilation_artifacts = source
        .compilation_artifacts
        .ok_or(anyhow::anyhow!("compilation artifacts are missing"))?;
    let creation_code_artifacts = source
        .creation_input_artifacts
        .ok_or(anyhow::anyhow!("creation code artifacts are missing"))?;
    let runtime_code_artifacts = source
        .deployed_bytecode_artifacts
        .ok_or(anyhow::anyhow!("runtime code artifacts are missing"))?;

    let creation_code_hash = insert_code(txn, source.raw_creation_input)
        .await
        .context("insert creation code")?
        .code_hash;
    let runtime_code_hash = insert_code(txn, source.raw_deployed_bytecode)
        .await
        .context("insert runtime code")?
        .code_hash;

    let active_model = compiled_contracts::ActiveModel {
        id: Default::default(),
        compiler: Set(compiler.to_string()),
        version: Set(source.compiler_version),
        language: Set(language.to_string()),
        name: Set(source.contract_name),
        fully_qualified_name: Set(fully_qualified_name),
        sources: Set(sources),
        compiler_settings: Set(source.compiler_settings),
        compilation_artifacts: Set(compilation_artifacts),
        creation_code_hash: Set(creation_code_hash.clone()),
        creation_code_artifacts: Set(creation_code_artifacts),
        runtime_code_hash: Set(runtime_code_hash.clone()),
        runtime_code_artifacts: Set(runtime_code_artifacts),
    };
    let (compiled_contract, _inserted) = insert_then_select!(
        txn,
        compiled_contracts,
        active_model,
        [
            (Compiler, compiler),
            (Language, language),
            (CreationCodeHash, creation_code_hash),
            (RuntimeCodeHash, runtime_code_hash)
        ]
    )?;

    Ok(compiled_contract)
}

async fn insert_code(
    txn: &DatabaseTransaction,
    code: Vec<u8>,
) -> Result<code::Model, anyhow::Error> {
    let code_hash = keccak_hash::keccak(&code);

    let active_model = code::ActiveModel {
        code_hash: Set(code_hash.0.to_vec()),
        code: Set(Some(code)),
    };
    let (code, _inserted) =
        insert_then_select!(txn, code, active_model, [(CodeHash, code_hash.0.to_vec())])?;

    Ok(code)
}
