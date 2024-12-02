use crate::helpers::insert_then_select;
use anyhow::{anyhow, Context, Error};
use blockscout_display_bytes::ToHex;
use sea_orm::{
    prelude::{Decimal, Uuid},
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
};
use sha2::{Digest, Sha256};
use sha3::Keccak256;
use std::collections::BTreeMap;
use verification_common_v1::verifier_alliance::Match;
use verifier_alliance_entity_v1::{
    code, compiled_contracts, compiled_contracts_sources, contract_deployments, contracts, sources,
    verified_contracts,
};

pub use crate::types::{
    CompiledContract, CompiledContractCompiler, CompiledContractLanguage, ContractCode,
    InsertContractDeployment, RetrieveContractDeployment, VerifiedContract,
    VerifiedContractMatches,
};

#[derive(Clone, Debug)]
pub struct InternalContractDeploymentData {
    pub chain_id: Decimal,
    pub address: Vec<u8>,
    pub transaction_hash: Vec<u8>,
    pub block_number: Decimal,
    pub transaction_index: Decimal,
    pub deployer: Vec<u8>,
    pub contract_code: ContractCode,
}

impl From<InsertContractDeployment> for InternalContractDeploymentData {
    fn from(value: InsertContractDeployment) -> Self {
        match value {
            InsertContractDeployment::Genesis { .. } => parse_genesis_contract_deployment(value),
            InsertContractDeployment::Regular { .. } => parse_regular_contract_deployment(value),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct InternalMatchData {
    does_match: bool,
    metadata_match: Option<bool>,
    values: Option<serde_json::Value>,
    transformations: Option<serde_json::Value>,
}

pub async fn insert_verified_contract<C: ConnectionTrait>(
    database_connection: &C,
    contract_deployment_id: Uuid,
    compiled_contract_id: Uuid,
    matches: VerifiedContractMatches,
) -> Result<verified_contracts::Model, Error> {
    let (creation_match_data, runtime_match_data) = match matches {
        VerifiedContractMatches::OnlyRuntime { runtime_match } => {
            let runtime_match_data = parse_verification_common_match(runtime_match);
            (InternalMatchData::default(), runtime_match_data)
        }
        VerifiedContractMatches::OnlyCreation { creation_match } => {
            let creation_match_data = parse_verification_common_match(creation_match);
            (creation_match_data, InternalMatchData::default())
        }
        VerifiedContractMatches::Complete {
            runtime_match,
            creation_match,
        } => {
            let creation_match_data = parse_verification_common_match(creation_match);
            let runtime_match_data = parse_verification_common_match(runtime_match);
            (creation_match_data, runtime_match_data)
        }
    };

    let active_model = verified_contracts::ActiveModel {
        id: Default::default(),
        created_at: Default::default(),
        updated_at: Default::default(),
        created_by: Default::default(),
        updated_by: Default::default(),
        deployment_id: Set(contract_deployment_id),
        compilation_id: Set(compiled_contract_id),
        creation_match: Set(creation_match_data.does_match),
        creation_values: Set(creation_match_data.values),
        creation_transformations: Set(creation_match_data.transformations),
        creation_metadata_match: Set(creation_match_data.metadata_match),
        runtime_match: Set(runtime_match_data.does_match),
        runtime_values: Set(runtime_match_data.values),
        runtime_transformations: Set(runtime_match_data.transformations),
        runtime_metadata_match: Set(runtime_match_data.metadata_match),
    };

    let (model, _inserted) = insert_then_select!(
        database_connection,
        verified_contracts,
        active_model,
        false,
        [
            (CompilationId, compiled_contract_id),
            (DeploymentId, contract_deployment_id),
        ]
    )?;

    Ok(model)
}

pub async fn insert_compiled_contract<C: ConnectionTrait>(
    database_connection: &C,
    compiled_contract: CompiledContract,
) -> Result<compiled_contracts::Model, Error> {
    let creation_code_hash = insert_code(database_connection, compiled_contract.creation_code)
        .await
        .context("insert creation code")?
        .code_hash;
    let runtime_code_hash = insert_code(database_connection, compiled_contract.runtime_code)
        .await
        .context("insert runtime code")?
        .code_hash;

    let active_model = compiled_contracts::ActiveModel {
        id: Default::default(),
        created_at: Default::default(),
        updated_at: Default::default(),
        created_by: Default::default(),
        updated_by: Default::default(),
        compiler: Set(compiled_contract.compiler.to_string()),
        version: Set(compiled_contract.version),
        language: Set(compiled_contract.language.to_string()),
        name: Set(compiled_contract.name),
        fully_qualified_name: Set(compiled_contract.fully_qualified_name),
        compiler_settings: Set(compiled_contract.compiler_settings),
        compilation_artifacts: Set(compiled_contract.compilation_artifacts.into()),
        creation_code_hash: Set(creation_code_hash.clone()),
        creation_code_artifacts: Set(compiled_contract.creation_code_artifacts.into()),
        runtime_code_hash: Set(runtime_code_hash.clone()),
        runtime_code_artifacts: Set(compiled_contract.runtime_code_artifacts.into()),
    };

    let (model, _inserted) = insert_then_select!(
        database_connection,
        compiled_contracts,
        active_model,
        false,
        [
            (Compiler, compiled_contract.compiler.to_string()),
            (Language, compiled_contract.language.to_string()),
            (CreationCodeHash, creation_code_hash),
            (RuntimeCodeHash, runtime_code_hash)
        ]
    )?;

    Ok(model)
}

pub async fn insert_sources<C: ConnectionTrait>(
    database_connection: &C,
    sources: BTreeMap<String, String>,
) -> Result<Vec<sources::Model>, Error> {
    let mut models = vec![];

    for (_path, content) in sources {
        let source_hash = Sha256::digest(&content).to_vec();
        let source_hash_keccak = Keccak256::digest(&content).to_vec();
        let active_model = sources::ActiveModel {
            source_hash: Set(source_hash.clone()),
            source_hash_keccak: Set(source_hash_keccak),
            content: Set(content),
            created_at: Default::default(),
            updated_at: Default::default(),
            created_by: Default::default(),
            updated_by: Default::default(),
        };
        let (model, _inserted) = insert_then_select!(
            database_connection,
            sources,
            active_model,
            false,
            [(SourceHash, source_hash)]
        )?;
        models.push(model)
    }

    Ok(models)
}

pub async fn insert_compiled_contract_sources<C: ConnectionTrait>(
    database_connection: &C,
    source_hashes: BTreeMap<String, Vec<u8>>,
    compiled_contract_id: Uuid,
) -> Result<Vec<compiled_contracts_sources::Model>, Error> {
    let mut models = vec![];

    for (path, source_hash) in source_hashes {
        let active_model = compiled_contracts_sources::ActiveModel {
            id: Default::default(),
            compilation_id: Set(compiled_contract_id),
            path: Set(path.clone()),
            source_hash: Set(source_hash),
        };
        let (model, _inserted) = insert_then_select!(
            database_connection,
            compiled_contracts_sources,
            active_model,
            false,
            [(CompilationId, compiled_contract_id), (Path, path)]
        )?;
        models.push(model);
    }

    Ok(models)
}

pub async fn insert_contract_deployment<C: ConnectionTrait>(
    database_connection: &C,
    internal_data: InternalContractDeploymentData,
    contract_id: Uuid,
) -> Result<contract_deployments::Model, Error> {
    let active_model = contract_deployments::ActiveModel {
        id: Default::default(),
        created_at: Default::default(),
        updated_at: Default::default(),
        created_by: Default::default(),
        updated_by: Default::default(),
        chain_id: Set(internal_data.chain_id),
        address: Set(internal_data.address.clone()),
        transaction_hash: Set(internal_data.transaction_hash.clone()),
        block_number: Set(internal_data.block_number),
        transaction_index: Set(internal_data.transaction_index),
        deployer: Set(internal_data.deployer),
        contract_id: Set(contract_id),
    };

    let (model, _inserted) = insert_then_select!(
        database_connection,
        contract_deployments,
        active_model,
        false,
        [
            (ChainId, internal_data.chain_id),
            (Address, internal_data.address),
            (TransactionHash, internal_data.transaction_hash)
        ]
    )?;

    Ok(model)
}

pub async fn retrieve_contract_deployment<C: ConnectionTrait>(
    database_connection: &C,
    contract_deployment: RetrieveContractDeployment,
) -> Result<Option<contract_deployments::Model>, Error> {
    let transaction_hash = contract_deployment.transaction_hash.unwrap_or_else(|| {
        let runtime_code = contract_deployment
            .runtime_code
            .expect("either transaction hash or runtime code must contain value");
        calculate_genesis_contract_deployment_transaction_hash(&runtime_code)
    });

    contract_deployments::Entity::find()
        .filter(
            contract_deployments::Column::ChainId.eq(Decimal::from(contract_deployment.chain_id)),
        )
        .filter(contract_deployments::Column::Address.eq(contract_deployment.address))
        .filter(contract_deployments::Column::TransactionHash.eq(transaction_hash))
        .one(database_connection)
        .await
        .context("select from \"contract_deployments\"")
}

pub async fn insert_contract<C: ConnectionTrait>(
    database_connection: &C,
    contract_code: ContractCode,
) -> Result<contracts::Model, Error> {
    let (creation_code_hash, runtime_code_hash) =
        insert_contract_code(database_connection, contract_code).await?;

    let active_model = contracts::ActiveModel {
        id: Default::default(),
        created_at: Default::default(),
        updated_at: Default::default(),
        created_by: Default::default(),
        updated_by: Default::default(),
        creation_code_hash: Set(creation_code_hash.clone()),
        runtime_code_hash: Set(runtime_code_hash.clone()),
    };

    let (model, _inserted) = insert_then_select!(
        database_connection,
        contracts,
        active_model,
        false,
        [
            (CreationCodeHash, creation_code_hash),
            (RuntimeCodeHash, runtime_code_hash)
        ]
    )?;

    Ok(model)
}

pub async fn insert_code<C: ConnectionTrait>(
    database_connection: &C,
    code: Vec<u8>,
) -> Result<code::Model, Error> {
    let code_hash = Sha256::digest(&code).to_vec();
    let code_hash_keccak = Keccak256::digest(&code).to_vec();

    let active_model = code::ActiveModel {
        code_hash: Set(code_hash.clone()),
        created_at: Default::default(),
        updated_at: Default::default(),
        created_by: Default::default(),
        updated_by: Default::default(),
        code_hash_keccak: Set(code_hash_keccak),
        code: Set(Some(code)),
    };

    let (model, _inserted) = insert_then_select!(
        database_connection,
        code,
        active_model,
        false,
        [(CodeHash, code_hash)]
    )?;

    Ok(model)
}

/// Inserts a contract defined by its runtime and creation code into `contracts` table.
/// Notice, that only creation code is optional, while runtime code should always exist.
/// It can be empty though, in case creation code execution resulted in empty code.
/// Creation code may be missed for genesis contracts.
async fn insert_contract_code<C: ConnectionTrait>(
    database_connection: &C,
    contract_code: ContractCode,
) -> Result<(Vec<u8>, Vec<u8>), Error> {
    let mut creation_code_hash = vec![];
    let runtime_code_hash;

    match contract_code {
        ContractCode::OnlyRuntimeCode { code } => {
            runtime_code_hash = insert_code(database_connection, code)
                .await
                .context("insert runtime code")?
                .code_hash;
        }
        ContractCode::CompleteCode {
            creation_code,
            runtime_code,
        } => {
            creation_code_hash = insert_code(database_connection, creation_code)
                .await
                .context("insert creation code")?
                .code_hash;
            runtime_code_hash = insert_code(database_connection, runtime_code)
                .await
                .context("insert runtime code")?
                .code_hash;
        }
    }

    Ok((creation_code_hash, runtime_code_hash))
}

pub async fn retrieve_contract_by_id<C: ConnectionTrait>(
    database_connection: &C,
    contract_id: Uuid,
) -> Result<contracts::Model, Error> {
    contracts::Entity::find_by_id(contract_id)
        .one(database_connection)
        .await
        .context("select from \"contracts\" by id")?
        .ok_or_else(|| anyhow!("contract id was not found: {}", contract_id))
}

pub async fn retrieve_code_by_id<C: ConnectionTrait>(
    database_connection: &C,
    code_hash: Vec<u8>,
) -> Result<code::Model, Error> {
    code::Entity::find_by_id(code_hash.clone())
        .one(database_connection)
        .await
        .context("select from \"code\" by id")?
        .ok_or_else(|| anyhow!("code hash was not found: {}", code_hash.to_hex()))
}

pub fn precalculate_source_hashes(sources: &BTreeMap<String, String>) -> BTreeMap<String, Vec<u8>> {
    let mut source_hashes = BTreeMap::new();
    for (path, content) in sources {
        let source_hash = Sha256::digest(content).to_vec();
        source_hashes.insert(path.clone(), source_hash);
    }

    source_hashes
}

fn parse_genesis_contract_deployment(
    contract_deployment: InsertContractDeployment,
) -> InternalContractDeploymentData {
    if let InsertContractDeployment::Genesis {
        chain_id,
        address,
        runtime_code,
    } = contract_deployment
    {
        let transaction_hash =
            calculate_genesis_contract_deployment_transaction_hash(&runtime_code);
        let contract_code = ContractCode::OnlyRuntimeCode { code: runtime_code };

        return InternalContractDeploymentData {
            chain_id: Decimal::from(chain_id),
            address,
            transaction_hash,
            block_number: Decimal::from(-1),
            transaction_index: Decimal::from(-1),
            deployer: vec![],
            contract_code,
        };
    }

    unreachable!()
}

fn parse_regular_contract_deployment(
    contract_deployment: InsertContractDeployment,
) -> InternalContractDeploymentData {
    if let InsertContractDeployment::Regular {
        chain_id,
        address,
        transaction_hash,
        block_number,
        transaction_index,
        deployer,
        creation_code,
        runtime_code,
    } = contract_deployment
    {
        let contract_code = ContractCode::CompleteCode {
            creation_code,
            runtime_code,
        };

        return InternalContractDeploymentData {
            chain_id: Decimal::from(chain_id),
            address,
            transaction_hash,
            block_number: Decimal::from(block_number),
            transaction_index: Decimal::from(transaction_index),
            deployer,
            contract_code,
        };
    }

    unreachable!()
}

fn calculate_genesis_contract_deployment_transaction_hash(runtime_code: &[u8]) -> Vec<u8> {
    Keccak256::digest(runtime_code).to_vec()
}

fn parse_verification_common_match(match_value: Match) -> InternalMatchData {
    InternalMatchData {
        does_match: true,
        metadata_match: Some(match_value.metadata_match),
        values: Some(match_value.values.into()),
        transformations: Some(match_value.transformations.into()),
    }
}
