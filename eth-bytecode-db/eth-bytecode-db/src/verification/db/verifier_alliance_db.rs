use super::{
    super::{types, verifier_alliance, SourceType},
    insert_then_select,
};
use anyhow::Context;
use blockscout_display_bytes::Bytes as DisplayBytes;
use sea_orm::{
    entity::prelude::ColumnTrait, ActiveValue::Set, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, TransactionTrait,
};
use verifier_alliance_entity::{
    code, compiled_contracts, contract_deployments, contracts, verified_contracts,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct ContractDeploymentData {
    pub chain_id: i64,
    pub contract_address: Vec<u8>,
    pub transaction_hash: Vec<u8>,
    pub block_number: Option<i64>,
    pub transaction_index: Option<i64>,
    pub deployer: Option<Vec<u8>>,
    pub creation_code: Option<Vec<u8>>,
    pub runtime_code: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
enum TransformationStatus {
    NoMatch,
    WithAuxdata,
    WithoutAuxdata,
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

    let contract_deployment = retrieve_contract_deployment(&txn, &deployment_data)
        .await
        .context("retrieve contract contract_deployment")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "contract deployment was not found: chain_id={}, address={}, transaction_hash={}",
                deployment_data.chain_id,
                DisplayBytes::from(deployment_data.contract_address.clone()),
                DisplayBytes::from(deployment_data.transaction_hash.clone())
            )
        })?;

    let contract = retrieve_contract(&txn, &contract_deployment)
        .await
        .context("retrieve contract")?;

    let deployment_verified_contracts =
        retrieve_deployment_verified_contracts(&txn, &contract_deployment)
            .await
            .context("retrieve deployment verified contracts")?;
    let max_statuses = deployment_verified_contracts
        .iter()
        .map(retrieve_transformation_statuses)
        .fold(
            (TransformationStatus::NoMatch, TransformationStatus::NoMatch),
            |statuses, current_status| {
                let creation_code_status = std::cmp::max(statuses.0, current_status.0);
                let runtime_code_status = std::cmp::max(statuses.1, current_status.1);

                (creation_code_status, runtime_code_status)
            },
        );

    let compiled_contract = insert_compiled_contract(&txn, source_response)
        .await
        .context("insert compiled_contract")?;

    let _verified_contract = insert_verified_contract(
        &deployment_data,
        &txn,
        &contract,
        &contract_deployment,
        &compiled_contract,
        max_statuses,
    )
    .await
    .context("insert verified_contract")?;

    txn.commit().await.context("commit transaction")?;

    Ok(())
}

pub(crate) async fn insert_deployment_data(
    db_client: &DatabaseConnection,
    mut deployment_data: ContractDeploymentData,
) -> Result<(), anyhow::Error> {
    let txn = db_client
        .begin()
        .await
        .context("begin database transaction")?;

    let contract = insert_contract(
        &txn,
        deployment_data.creation_code.take(),
        deployment_data.runtime_code.take(),
    )
    .await
    .context("insert contract")?;

    let _contract_deployment = insert_contract_deployment(&txn, deployment_data, &contract)
        .await
        .context("insert contract deployment")?;

    txn.commit().await.context("commit transaction")?;

    Ok(())
}

async fn retrieve_contract_deployment(
    txn: &DatabaseTransaction,
    deployment_data: &ContractDeploymentData,
) -> Result<Option<contract_deployments::Model>, anyhow::Error> {
    contract_deployments::Entity::find()
        .filter(contract_deployments::Column::ChainId.eq(deployment_data.chain_id))
        .filter(contract_deployments::Column::Address.eq(deployment_data.contract_address.clone()))
        .filter(
            contract_deployments::Column::TransactionHash
                .eq(deployment_data.transaction_hash.clone()),
        )
        .one(txn)
        .await
        .context("select from \"contract_deployments\"")
}

async fn retrieve_deployment_verified_contracts(
    txn: &DatabaseTransaction,
    contract_deployment: &contract_deployments::Model,
) -> Result<Vec<verified_contracts::Model>, anyhow::Error> {
    verified_contracts::Entity::find()
        .filter(verified_contracts::Column::DeploymentId.eq(contract_deployment.id))
        .all(txn)
        .await
        .context("select from \"verified_contracts\" by deployment id")
}

async fn retrieve_contract(
    txn: &DatabaseTransaction,
    contract_deployment: &contract_deployments::Model,
) -> Result<contracts::Model, anyhow::Error> {
    contracts::Entity::find_by_id(contract_deployment.contract_id)
        .one(txn)
        .await
        .context("select from \"contracts\" by id")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "contract was not found, though referring contract deployment exists; contract_id={}",
                contract_deployment.contract_id
            )
        })
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
    deployment_data: &ContractDeploymentData,
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
                    let contract_address =
                        DisplayBytes::from(deployment_data.contract_address.clone());
                    tracing::warn!(
                        contract_address = contract_address.to_string(),
                        chain_id = deployment_data.chain_id,
                        "code processing failed; err={err:#}"
                    );
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

fn retrieve_transformation_statuses(
    verified_contract: &verified_contracts::Model,
) -> (TransformationStatus, TransformationStatus) {
    let creation_code_status = retrieve_code_transformation_status(
        Some(verified_contract.id),
        true,
        verified_contract.creation_match,
        verified_contract.creation_values.as_ref(),
    );
    let runtime_code_status = retrieve_code_transformation_status(
        Some(verified_contract.id),
        false,
        verified_contract.runtime_match,
        verified_contract.runtime_values.as_ref(),
    );

    (creation_code_status, runtime_code_status)
}

fn retrieve_code_transformation_status(
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

async fn insert_verified_contract(
    deployment_data: &ContractDeploymentData,
    txn: &DatabaseTransaction,
    contract: &contracts::Model,
    contract_deployment: &contract_deployments::Model,
    compiled_contract: &compiled_contracts::Model,
    (existing_creation_code_status, existing_runtime_code_status): (
        TransformationStatus,
        TransformationStatus,
    ),
) -> Result<verified_contracts::Model, anyhow::Error> {
    let (creation_match, creation_values, creation_transformations) = check_code_match(
        deployment_data,
        txn,
        contract.creation_code_hash.clone(),
        compiled_contract.creation_code_hash.clone(),
        compiled_contract.creation_code_artifacts.clone(),
        verifier_alliance::process_creation_code,
    )
    .await
    .context("check creation code match")?;
    let (runtime_match, runtime_values, runtime_transformations) = check_code_match(
        deployment_data,
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
    }

    let creation_code_status =
        retrieve_code_transformation_status(None, true, creation_match, creation_values.as_ref());
    let runtime_code_status =
        retrieve_code_transformation_status(None, false, runtime_match, runtime_values.as_ref());
    if existing_creation_code_status >= creation_code_status
        && existing_runtime_code_status >= runtime_code_status
    {
        return Err(anyhow::anyhow!(
            "New verified contract is not better than existing for the given contract deployment"
        ));
    }

    let active_model = verified_contracts::ActiveModel {
        id: Default::default(),
        created_at: Default::default(),
        updated_at: Default::default(),
        deployment_id: Set(contract_deployment.id),
        compilation_id: Set(compiled_contract.id),
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
        false,
        [
            (CompilationId, compiled_contract.id),
            (DeploymentId, contract_deployment.id),
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
        created_at: Default::default(),
        updated_at: Default::default(),
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
        false,
        [
            (Compiler, compiler),
            (Language, language),
            (CreationCodeHash, creation_code_hash),
            (RuntimeCodeHash, runtime_code_hash)
        ]
    )?;

    Ok(compiled_contract)
}

async fn insert_contract_deployment(
    txn: &DatabaseTransaction,
    deployment_data: ContractDeploymentData,
    contract: &contracts::Model,
) -> Result<contract_deployments::Model, anyhow::Error> {
    let active_model = contract_deployments::ActiveModel {
        id: Default::default(),
        chain_id: Set(deployment_data.chain_id.into()),
        address: Set(deployment_data.contract_address.clone()),
        transaction_hash: Set(deployment_data.transaction_hash.clone()),
        block_number: Set(deployment_data.block_number.unwrap_or(-1).into()),
        txindex: Set(deployment_data.transaction_index.unwrap_or(-1).into()),
        deployer: Set(deployment_data
            .deployer
            .unwrap_or(ethers_core::types::Address::zero().0.to_vec())),
        contract_id: Set(contract.id),
    };
    let (contract_deployment, _inserted) = insert_then_select!(
        txn,
        contract_deployments,
        active_model,
        false,
        [
            (ChainId, deployment_data.chain_id),
            (Address, deployment_data.contract_address),
            (TransactionHash, deployment_data.transaction_hash)
        ]
    )?;

    Ok(contract_deployment)
}

async fn insert_contract(
    txn: &DatabaseTransaction,
    creation_code: Option<Vec<u8>>,
    runtime_code: Option<Vec<u8>>,
) -> Result<contracts::Model, anyhow::Error> {
    if creation_code.is_none() && runtime_code.is_none() {
        return Err(anyhow::anyhow!(
            "at least one of creation or runtime code must not be null"
        ));
    }
    let creation_code = if let Some(creation_code) = creation_code {
        Some(
            insert_code(txn, creation_code)
                .await
                .context("insert creation code")?,
        )
    } else {
        None
    };
    let runtime_code = if let Some(runtime_code) = runtime_code {
        Some(
            insert_code(txn, runtime_code)
                .await
                .context("insert runtime code")?,
        )
    } else {
        None
    };

    let creation_code_hash = creation_code.map(|code| code.code_hash).unwrap_or_default();
    let runtime_code_hash = runtime_code.map(|code| code.code_hash).unwrap_or_default();

    let active_model = contracts::ActiveModel {
        id: Default::default(),
        creation_code_hash: Set(creation_code_hash.clone()),
        runtime_code_hash: Set(runtime_code_hash.clone()),
    };
    let (contract, _inserted) = insert_then_select!(
        txn,
        contracts,
        active_model,
        false,
        [
            (CreationCodeHash, creation_code_hash),
            (RuntimeCodeHash, runtime_code_hash)
        ]
    )?;

    Ok(contract)
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
    let (code, _inserted) = insert_then_select!(
        txn,
        code,
        active_model,
        false,
        [(CodeHash, code_hash.0.to_vec())]
    )?;

    Ok(code)
}
