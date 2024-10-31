mod helpers;
mod types;

use crate::helpers::insert_then_select;
use anyhow::Context;
use anyhow::Error;
use sea_orm::ActiveValue::Set;
use sea_orm::ConnectionTrait;
use sha2::{Digest, Sha256};
use sha3::Keccak256;
use verifier_alliance_entity::{code, contracts};

pub use types::ContractCode;

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
    let mut runtime_code_hash = vec![];

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
