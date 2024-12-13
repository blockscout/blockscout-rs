use super::{
    super::{types, verifier_alliance, SourceType},
    insert_then_select,
};
use anyhow::Context;
use sea_orm::{
    entity::prelude::ColumnTrait, ActiveValue::Set, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, TransactionTrait,
};
use verifier_alliance_database::{
    CompiledContract, CompiledContractCompiler, CompiledContractLanguage,
};
use verifier_alliance_entity::{
    code, compiled_contracts, contract_deployments, contracts, verified_contracts,
};

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
    let mut creation_metadata_match = None;
    if creation_code_match.does_match {
        creation_metadata_match = Some(false);
    }
    let mut runtime_metadata_match = None;
    if runtime_code_match.does_match {
        runtime_metadata_match = Some(false);
    }
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
        creation_metadata_match: Set(creation_metadata_match),
        runtime_match: Set(runtime_code_match.does_match),
        runtime_values: Set(runtime_code_match.values),
        runtime_transformations: Set(runtime_code_match.transformations),
        runtime_metadata_match: Set(runtime_metadata_match),
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
    let compilation_artifacts = source
        .compilation_artifacts
        .ok_or(anyhow::anyhow!("compilation artifacts are missing"))?;
    let creation_code_artifacts = source
        .creation_code_artifacts
        .ok_or(anyhow::anyhow!("creation code artifacts are missing"))?;
    let runtime_code_artifacts = source
        .runtime_code_artifacts
        .ok_or(anyhow::anyhow!("runtime code artifacts are missing"))?;

    let (compiler, language) = match source.source_type {
        SourceType::Solidity => (
            CompiledContractCompiler::Solc,
            CompiledContractLanguage::Solidity,
        ),
        SourceType::Vyper => (
            CompiledContractCompiler::Vyper,
            CompiledContractLanguage::Vyper,
        ),
        SourceType::Yul => (
            CompiledContractCompiler::Solc,
            CompiledContractLanguage::Yul,
        ),
    };

    let fully_qualified_name = format!("{}:{}", source.file_name, source.contract_name);

    let compiled_contract = CompiledContract {
        compiler,
        version: source.compiler_version,
        language,
        name: source.contract_name,
        fully_qualified_name,
        sources: source.source_files,
        compiler_settings: source.compiler_settings,
        compilation_artifacts,
        creation_code: source.raw_creation_code,
        creation_code_artifacts,
        runtime_code: source.raw_runtime_code,
        runtime_code_artifacts,
    };
    let model =
        verifier_alliance_database::internal::insert_compiled_contract(db, compiled_contract)
            .await?;

    Ok(model)
}
