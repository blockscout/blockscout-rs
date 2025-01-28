use crate::helpers;
use anyhow::{anyhow, Context, Error};
use blockscout_display_bytes::ToHex;
use sea_orm::{
    prelude::{Decimal, Uuid},
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sha3::Keccak256;
use std::{collections::BTreeMap, str::FromStr};
use verification_common::verifier_alliance::Match;
use verifier_alliance_entity::{
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

    use verified_contracts::Column;
    let (model, _inserted) = helpers::insert_then_select(
        database_connection,
        verified_contracts::Entity,
        active_model,
        [
            (Column::CompilationId, compiled_contract_id.into()),
            (Column::DeploymentId, contract_deployment_id.into()),
        ],
    )
    .await?;

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

    use compiled_contracts::Column;
    let (model, _inserted) = helpers::insert_then_select(
        database_connection,
        compiled_contracts::Entity,
        active_model,
        [
            (
                Column::Compiler,
                compiled_contract.compiler.to_string().into(),
            ),
            (
                Column::Language,
                compiled_contract.language.to_string().into(),
            ),
            (Column::CreationCodeHash, creation_code_hash.into()),
            (Column::RuntimeCodeHash, runtime_code_hash.into()),
        ],
    )
    .await?;

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
        let (model, _inserted) = helpers::insert_then_select(
            database_connection,
            sources::Entity,
            active_model,
            [(sources::Column::SourceHash, source_hash.into())],
        )
        .await?;
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
        use compiled_contracts_sources::Column;
        let (model, _inserted) = helpers::insert_then_select(
            database_connection,
            compiled_contracts_sources::Entity,
            active_model,
            [
                (Column::CompilationId, compiled_contract_id.into()),
                (Column::Path, path.into()),
            ],
        )
        .await?;
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

    use contract_deployments::Column;
    let (model, _inserted) = helpers::insert_then_select(
        database_connection,
        contract_deployments::Entity,
        active_model,
        [
            (Column::ChainId, internal_data.chain_id.into()),
            (Column::Address, internal_data.address.into()),
            (
                Column::TransactionHash,
                internal_data.transaction_hash.into(),
            ),
        ],
    )
    .await?;

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

pub async fn retrieve_contract_deployments_by_chain_id_and_address<C: ConnectionTrait>(
    database_connection: &C,
    chain_id: u128,
    contract_address: Vec<u8>,
) -> Result<Vec<contract_deployments::Model>, Error> {
    contract_deployments::Entity::find()
        .filter(contract_deployments::Column::ChainId.eq(Decimal::from(chain_id)))
        .filter(contract_deployments::Column::Address.eq(contract_address))
        .all(database_connection)
        .await
        .context("select from \"contract_deployments\"")
}

pub async fn retrieve_verified_contracts_by_deployment_id<C: ConnectionTrait>(
    database_connection: &C,
    deployment_id: Uuid,
) -> Result<Vec<verified_contracts::Model>, Error> {
    verified_contracts::Entity::find()
        .filter(verified_contracts::Column::DeploymentId.eq(deployment_id))
        .all(database_connection)
        .await
        .context("select from \"verified_contracts\"")
}

pub async fn retrieve_compiled_contract_by_id<C: ConnectionTrait>(
    database_connection: &C,
    id: Uuid,
) -> Result<Option<compiled_contracts::Model>, Error> {
    compiled_contracts::Entity::find_by_id(id)
        .one(database_connection)
        .await
        .context("select from \"compiled_contracts\"")
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

    use contracts::Column;
    let (model, _inserted) = helpers::insert_then_select(
        database_connection,
        contracts::Entity,
        active_model,
        [
            (Column::CreationCodeHash, creation_code_hash.into()),
            (Column::RuntimeCodeHash, runtime_code_hash.into()),
        ],
    )
    .await?;

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

    let (model, _inserted) = helpers::insert_then_select(
        database_connection,
        code::Entity,
        active_model,
        [(code::Column::CodeHash, code_hash.into())],
    )
    .await?;

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

pub async fn retrieve_sources_by_compilation_id<C: ConnectionTrait>(
    database_connection: &C,
    compilation_id: Uuid,
) -> Result<BTreeMap<String, String>, Error> {
    let compiled_contract_source_models = compiled_contracts_sources::Entity::find()
        .filter(compiled_contracts_sources::Column::CompilationId.eq(compilation_id))
        .all(database_connection)
        .await
        .context("select from \"compiled_contracts_sources\"")?;

    let mut sources = BTreeMap::new();
    for compiled_contract_source_model in compiled_contract_source_models {
        let source_model =
            sources::Entity::find_by_id(compiled_contract_source_model.source_hash.clone())
                .one(database_connection)
                .await
                .context("select from \"sources\"")?
                .ok_or_else(|| {
                    anyhow!(
                        "source hash does not exist: {}",
                        compiled_contract_source_model.source_hash.to_hex()
                    )
                })?;

        sources.insert(compiled_contract_source_model.path, source_model.content);
    }

    Ok(sources)
}

pub fn precalculate_source_hashes(sources: &BTreeMap<String, String>) -> BTreeMap<String, Vec<u8>> {
    let mut source_hashes = BTreeMap::new();
    for (path, content) in sources {
        let source_hash = Sha256::digest(content).to_vec();
        source_hashes.insert(path.clone(), source_hash);
    }

    source_hashes
}

pub fn try_models_into_verified_contract(
    contract_deployment_id: Uuid,
    compiled_contract: compiled_contracts::Model,
    creation_code: Vec<u8>,
    runtime_code: Vec<u8>,
    sources: BTreeMap<String, String>,
    verified_contract: verified_contracts::Model,
) -> Result<VerifiedContract, Error> {
    let compilation_artifacts = serde_json::from_value(compiled_contract.compilation_artifacts)
        .context("parsing compilation artifacts")?;
    let creation_code_artifacts = serde_json::from_value(compiled_contract.creation_code_artifacts)
        .context("parsing creation code artifacts")?;
    let runtime_code_artifacts = serde_json::from_value(compiled_contract.runtime_code_artifacts)
        .context("parsing runtime code artifacts")?;

    let compiler = CompiledContractCompiler::from_str(&compiled_contract.compiler.to_lowercase())
        .context("parsing compiler")?;
    let language = CompiledContractLanguage::from_str(&compiled_contract.language.to_lowercase())
        .context("parsing language")?;

    // We can safely unwrap thanks to `verified_contracts_creation_match_integrity` and `verified_contracts_runtime_match_integrity` database constraints
    let creation_match = verified_contract
        .creation_match
        .then(|| {
            extract_match_from_model(
                verified_contract.creation_metadata_match.unwrap(),
                verified_contract.creation_transformations.unwrap(),
                verified_contract.creation_values.unwrap(),
            )
        })
        .transpose()?;
    let runtime_match = verified_contract
        .runtime_match
        .then(|| {
            extract_match_from_model(
                verified_contract.runtime_metadata_match.unwrap(),
                verified_contract.runtime_transformations.unwrap(),
                verified_contract.runtime_values.unwrap(),
            )
        })
        .transpose()?;

    let matches = match (creation_match, runtime_match) {
        (Some(creation_match), Some(runtime_match)) => VerifiedContractMatches::Complete {
            creation_match,
            runtime_match,
        },
        (Some(creation_match), None) => VerifiedContractMatches::OnlyCreation { creation_match },
        (None, Some(runtime_match)) => VerifiedContractMatches::OnlyRuntime { runtime_match },
        (None, None) => unreachable!("`verified_contracts_match_exists` database constraint"),
    };

    Ok(VerifiedContract {
        contract_deployment_id,
        compiled_contract: CompiledContract {
            compiler,
            version: compiled_contract.version,
            language,
            name: compiled_contract.name,
            fully_qualified_name: compiled_contract.fully_qualified_name,
            sources,
            compiler_settings: compiled_contract.compiler_settings,
            compilation_artifacts,
            creation_code,
            creation_code_artifacts,
            runtime_code,
            runtime_code_artifacts,
        },
        matches,
    })
}

pub use compare_matches::should_store_the_match;
mod compare_matches {
    use super::*;

    #[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
    enum MatchStatus {
        NoMatch,
        WithoutMetadata,
        WithMetadata,
    }

    impl From<&Match> for MatchStatus {
        fn from(value: &Match) -> Self {
            if value.metadata_match {
                MatchStatus::WithMetadata
            } else {
                MatchStatus::WithoutMetadata
            }
        }
    }

    fn status_from_model_match(does_match: bool, does_metadata_match: Option<bool>) -> MatchStatus {
        if !does_match {
            return MatchStatus::NoMatch;
        }
        if let Some(true) = does_metadata_match {
            return MatchStatus::WithMetadata;
        }
        MatchStatus::WithoutMetadata
    }

    pub async fn should_store_the_match<C: ConnectionTrait>(
        database_connection: &C,
        contract_deployment_id: Uuid,
        potential_matches: &VerifiedContractMatches,
    ) -> Result<bool, Error> {
        let (potential_creation_match, potential_runtime_match) = match potential_matches {
            VerifiedContractMatches::OnlyCreation { creation_match } => {
                (creation_match.into(), MatchStatus::NoMatch)
            }
            VerifiedContractMatches::OnlyRuntime { runtime_match } => {
                (MatchStatus::NoMatch, runtime_match.into())
            }
            VerifiedContractMatches::Complete {
                creation_match,
                runtime_match,
            } => (creation_match.into(), runtime_match.into()),
        };

        // We want to store all perfect matches even if there are other ones in the database.
        // That should be impossible, but in case that happens we are interested in storing them all
        // in order to manually review them later.
        if potential_creation_match == MatchStatus::WithMetadata
            || potential_runtime_match == MatchStatus::WithMetadata
        {
            return Ok(true);
        }

        let is_model_worse = |model: &verified_contracts::Model| {
            let model_creation_match =
                status_from_model_match(model.creation_match, model.creation_metadata_match);
            let model_runtime_match =
                status_from_model_match(model.runtime_match, model.runtime_metadata_match);
            model_creation_match < potential_creation_match
                || model_runtime_match < potential_runtime_match
        };
        let existing_verified_contracts = retrieve_verified_contracts_by_deployment_id(
            database_connection,
            contract_deployment_id,
        )
        .await?;
        let should_potential_match_be_stored =
            existing_verified_contracts.iter().all(is_model_worse);

        Ok(should_potential_match_be_stored)
    }
}

fn extract_match_from_model(
    metadata_match: bool,
    transformations: Value,
    values: Value,
) -> Result<Match, Error> {
    let transformations =
        serde_json::from_value(transformations).context("parsing match transformations")?;
    let values = serde_json::from_value(values).context("parsing match values")?;

    Ok(Match {
        metadata_match,
        transformations,
        values,
    })
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
