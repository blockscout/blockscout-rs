use super::{
    super::{types, verifier_alliance, SourceType},
    insert_then_select,
};
use anyhow::Context;
use sea_orm::{
    entity::prelude::ColumnTrait, ActiveValue::Set, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, TransactionTrait,
};
use sha3::{Digest, Keccak256, Sha3_256};
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

pub(crate) async fn insert_data(
    db_client: &DatabaseConnection,
    source_response: types::DatabaseReadySource,
    contract_deployment: contract_deployments::Model,
    creation_code_match: verifier_alliance::CodeMatch,
    runtime_code_match: verifier_alliance::CodeMatch,
) -> Result<(), anyhow::Error> {
    let txn = db_client
        .begin()
        .await
        .context("begin database transaction")?;

    let compiled_contract = insert_compiled_contract(&txn, source_response)
        .await
        .context("insert compiled_contract")?;

    let _verified_contract = insert_verified_contract(
        &txn,
        &contract_deployment,
        &compiled_contract,
        creation_code_match,
        runtime_code_match,
    )
    .await
    .context("insert verified_contract")?;

    txn.commit().await.context("commit transaction")?;

    Ok(())
}

pub(crate) async fn insert_deployment_data(
    db_client: &DatabaseConnection,
    mut deployment_data: ContractDeploymentData,
) -> Result<contract_deployments::Model, anyhow::Error> {
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

    let contract_deployment = insert_contract_deployment(&txn, deployment_data, &contract)
        .await
        .context("insert contract deployment")?;

    txn.commit().await.context("commit transaction")?;

    Ok(contract_deployment)
}

pub(crate) async fn retrieve_contract_deployment<C: ConnectionTrait>(
    db: &C,
    deployment_data: &ContractDeploymentData,
) -> Result<Option<contract_deployments::Model>, anyhow::Error> {
    contract_deployments::Entity::find()
        .filter(contract_deployments::Column::ChainId.eq(deployment_data.chain_id))
        .filter(contract_deployments::Column::Address.eq(deployment_data.contract_address.clone()))
        .filter(
            contract_deployments::Column::TransactionHash
                .eq(deployment_data.transaction_hash.clone()),
        )
        .one(db)
        .await
        .context("select from \"contract_deployments\"")
}

pub(crate) async fn retrieve_deployment_verified_contracts<C: ConnectionTrait>(
    db: &C,
    contract_deployment: &contract_deployments::Model,
) -> Result<Vec<verified_contracts::Model>, anyhow::Error> {
    verified_contracts::Entity::find()
        .filter(verified_contracts::Column::DeploymentId.eq(contract_deployment.id))
        .all(db)
        .await
        .context("select from \"verified_contracts\" by deployment id")
}

pub(crate) async fn retrieve_contract_codes<C: ConnectionTrait>(
    db: &C,
    contract_deployment: &contract_deployments::Model,
) -> Result<(code::Model, code::Model), anyhow::Error> {
    let contract = retrieve_contract(db, contract_deployment)
        .await
        .context("retrieve contract")?;
    let creation_code = retrieve_code(db, contract.creation_code_hash.clone())
        .await
        .context("retrieve creation code")?
        .expect(
            "\"contracts\".\"creation_code_hash\" has a foreign key constraint on \"code\".\"code_hash\"",
        );
    let runtime_code = retrieve_code(db, contract.runtime_code_hash.clone())
        .await
        .context("retrieve runtime code")?
        .expect(
            "\"contracts\".\"runtime_code_hash\" has a foreign key constraint on \"code\".\"code_hash\"",
        );

    Ok((creation_code, runtime_code))
}

pub(crate) async fn retrieve_contract<C: ConnectionTrait>(
    db: &C,
    contract_deployment: &contract_deployments::Model,
) -> Result<contracts::Model, anyhow::Error> {
    contracts::Entity::find_by_id(contract_deployment.contract_id)
        .one(db)
        .await
        .context("select from \"contracts\" by id")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "contract was not found, though referring contract deployment exists; contract_id={}",
                contract_deployment.contract_id
            )
        })
}

pub(crate) async fn retrieve_code<C: ConnectionTrait>(
    db: &C,
    code_hash: Vec<u8>,
) -> Result<Option<code::Model>, anyhow::Error> {
    code::Entity::find_by_id(code_hash)
        .one(db)
        .await
        .context("select from \"code\"")
}

async fn insert_verified_contract<C: ConnectionTrait>(
    db: &C,
    contract_deployment: &contract_deployments::Model,
    compiled_contract: &compiled_contracts::Model,
    creation_code_match: verifier_alliance::CodeMatch,
    runtime_code_match: verifier_alliance::CodeMatch,
) -> Result<verified_contracts::Model, anyhow::Error> {
    let active_model = verified_contracts::ActiveModel {
        id: Default::default(),
        created_at: Default::default(),
        updated_at: Default::default(),
        created_by: Default::default(),
        updated_by: Default::default(),
        deployment_id: Set(contract_deployment.id),
        compilation_id: Set(compiled_contract.id),
        creation_match: Set(creation_code_match.does_match),
        creation_values: Set(creation_code_match.values),
        creation_transformations: Set(creation_code_match.transformations),
        runtime_match: Set(runtime_code_match.does_match),
        runtime_values: Set(runtime_code_match.values),
        runtime_transformations: Set(runtime_code_match.transformations),
    };

    let (verified_contract, _inserted) = insert_then_select!(
        db,
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

async fn insert_compiled_contract<C: ConnectionTrait>(
    db: &C,
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
        .creation_code_artifacts
        .ok_or(anyhow::anyhow!("creation code artifacts are missing"))?;
    let runtime_code_artifacts = source
        .runtime_code_artifacts
        .ok_or(anyhow::anyhow!("runtime code artifacts are missing"))?;

    let creation_code_hash = insert_code(db, source.raw_creation_code)
        .await
        .context("insert creation code")?
        .code_hash;
    let runtime_code_hash = insert_code(db, source.raw_runtime_code)
        .await
        .context("insert runtime code")?
        .code_hash;

    let active_model = compiled_contracts::ActiveModel {
        id: Default::default(),
        created_at: Default::default(),
        updated_at: Default::default(),
        created_by: Default::default(),
        updated_by: Default::default(),
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
        db,
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

async fn insert_contract_deployment<C: ConnectionTrait>(
    db: &C,
    deployment_data: ContractDeploymentData,
    contract: &contracts::Model,
) -> Result<contract_deployments::Model, anyhow::Error> {
    let active_model = contract_deployments::ActiveModel {
        id: Default::default(),
        chain_id: Set(deployment_data.chain_id.into()),
        address: Set(deployment_data.contract_address.clone()),
        transaction_hash: Set(deployment_data.transaction_hash.clone()),
        block_number: Set(deployment_data.block_number.unwrap_or(-1).into()),
        transaction_index: Set(deployment_data.transaction_index.unwrap_or(-1).into()),
        deployer: Set(deployment_data
            .deployer
            .unwrap_or(ethers_core::types::Address::zero().0.to_vec())),
        contract_id: Set(contract.id),
    };
    let (contract_deployment, _inserted) = insert_then_select!(
        db,
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

async fn insert_contract<C: ConnectionTrait>(
    db: &C,
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
            insert_code(db, creation_code)
                .await
                .context("insert creation code")?,
        )
    } else {
        None
    };
    let runtime_code = if let Some(runtime_code) = runtime_code {
        Some(
            insert_code(db, runtime_code)
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
        db,
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

async fn insert_code<C: ConnectionTrait>(
    db: &C,
    code: Vec<u8>,
) -> Result<code::Model, anyhow::Error> {
    let code_hash = Sha3_256::digest(&code).to_vec();
    let code_hash_keccak = Keccak256::digest(&code).to_vec();

    let active_model = code::ActiveModel {
        code_hash: Set(code_hash.clone()),
        code_hash_keccak: Set(code_hash_keccak),
        code: Set(Some(code)),
    };
    let (code, _inserted) =
        insert_then_select!(db, code, active_model, false, [(CodeHash, code_hash)])?;

    Ok(code)
}
