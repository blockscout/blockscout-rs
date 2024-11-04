mod helpers;
mod types;

use crate::helpers::insert_then_select;
use anyhow::{Context, Error};
use sea_orm::{
    prelude::Decimal, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
};
use sha2::{Digest, Sha256};
use sha3::Keccak256;
use verifier_alliance_entity::{code, contract_deployments, contracts};

pub use crate::types::{ContractCode, ContractDeployment, RetrieveContractDeployment};

struct InternalContractDeploymentData {
    chain_id: Decimal,
    address: Vec<u8>,
    transaction_hash: Vec<u8>,
    block_number: Decimal,
    transaction_index: Decimal,
    deployer: Vec<u8>,
    contract_code: ContractCode,
}

pub async fn insert_contract_deployment<C: ConnectionTrait>(
    database_connection: &C,
    contract_deployment: ContractDeployment,
) -> Result<contract_deployments::Model, Error> {
    let data = match contract_deployment {
        ContractDeployment::Genesis { .. } => {
            parse_genesis_contract_deployment(contract_deployment)
        }
        ContractDeployment::Regular { .. } => {
            parse_regular_contract_deployment(contract_deployment)
        }
    };

    let contract_id = insert_contract(database_connection, data.contract_code)
        .await
        .context("insert into \"contract_deployments\"")?
        .id;

    let active_model = contract_deployments::ActiveModel {
        id: Default::default(),
        created_at: Default::default(),
        updated_at: Default::default(),
        created_by: Default::default(),
        updated_by: Default::default(),
        chain_id: Set(data.chain_id),
        address: Set(data.address.clone()),
        transaction_hash: Set(data.transaction_hash.clone()),
        block_number: Set(data.block_number),
        transaction_index: Set(data.transaction_index),
        deployer: Set(data.deployer),
        contract_id: Set(contract_id),
    };

    let (model, _inserted) = insert_then_select!(
        database_connection,
        contract_deployments,
        active_model,
        false,
        [
            (ChainId, data.chain_id),
            (Address, data.address),
            (TransactionHash, data.transaction_hash)
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

fn parse_genesis_contract_deployment(
    contract_deployment: ContractDeployment,
) -> InternalContractDeploymentData {
    if let ContractDeployment::Genesis {
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
    contract_deployment: ContractDeployment,
) -> InternalContractDeploymentData {
    if let ContractDeployment::Regular {
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
