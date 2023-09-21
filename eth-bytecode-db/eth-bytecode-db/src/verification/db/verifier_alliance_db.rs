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

async fn insert_verified_contract(
    txn: &DatabaseTransaction,
    contract: &contracts::Model,
    compiled_contract: &compiled_contracts::Model,
) -> Result<verified_contracts::Model, anyhow::Error> {
    let deployed_creation_code = retrieve_code(txn, contract.creation_code_hash.clone()).await.context("retrieve deployed creation code")?
        .expect("\"contracts\".\"creation_code_hash\" has a foreign key constraint on \"code\".\"code_hash\"");
    let compiled_creation_code = retrieve_code(txn, compiled_contract.creation_code_hash.clone()).await.context("retrieve compiled creation code")?
        .expect("\"compiled_contracts\".\"creation_code_hash\" has a foreign key constraint on \"code\".\"code_hash\"");

    let deployed_runtime_code = retrieve_code(txn, contract.runtime_code_hash.clone()).await.context("retrieve deployed runtime code")?
        .expect("\"contracts\".\"creation_code_hash\" has a foreign key constraint on \"code\".\"code_hash\"");
    let compiled_runtime_code = retrieve_code(txn, compiled_contract.runtime_code_hash.clone()).await.context("retrieve compiled runtime code")?
        .expect("\"compiled_contracts\".\"creation_code_hash\" has a foreign key constraint on \"code\".\"code_hash\"");

    let creation_code_match_details =
        match (deployed_creation_code.code, compiled_creation_code.code) {
            (Some(deployed_code), Some(compiled_code)) => {
                let code_artifacts = compiled_contract.creation_code_artifacts.clone();
                match verifier_alliance::process_creation_code(
                    &deployed_code,
                    compiled_code,
                    code_artifacts,
                ) {
                    Ok(res) => Some(res),
                    Err(err) => {
                        tracing::warn!("creation code processing failed; err={err:#}");
                        None
                    }
                }
            }
            _ => None,
        };
    let runtime_code_match_details = match (deployed_runtime_code.code, compiled_runtime_code.code)
    {
        (Some(deployed_code), Some(compiled_code)) => {
            let code_artifacts = compiled_contract.runtime_code_artifacts.clone();
            match verifier_alliance::process_runtime_code(
                &deployed_code,
                compiled_code,
                code_artifacts,
            ) {
                Ok(res) => Some(res),
                Err(err) => {
                    println!("runtime code processing failed; err={err:#}");
                    tracing::warn!("runtime code processing failed; err={err:#}");
                    None
                }
            }
        }
        _ => None,
    };

    let (creation_match, creation_values, creation_transformations) =
        match creation_code_match_details {
            None => (false, None, None),
            Some((values, transformations)) => (true, Some(values), Some(transformations)),
        };
    let (runtime_match, runtime_values, runtime_transformations) = match runtime_code_match_details
    {
        None => (false, None, None),
        Some((values, transformations)) => (true, Some(values), Some(transformations)),
    };

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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use pretty_assertions::assert_eq;
    use sea_orm::{DatabaseBackend, QueryTrait};
    use verifier_alliance_entity::code;

    #[tokio::test]
    async fn test() {
        let chain_id: i64 = 123;
        let address = Bytes::from_static(&[1u8, 2, 3, 4]);
        let transaction_hash = Bytes::from_static(&[5u8, 6, 7, 8, 9, 10]);

        let query = contracts::Entity::find()
            .join(
                JoinType::Join,
                contracts::Relation::ContractDeployments.def(),
            )
            .filter(contract_deployments::Column::ChainId.eq(chain_id))
            .filter(contract_deployments::Column::Address.eq(address.to_vec()))
            .filter(contract_deployments::Column::TransactionHash.eq(transaction_hash.to_vec()))
            .build(DatabaseBackend::Postgres)
            .to_string();

        assert_eq!("", query);

        let query = code::Entity::find()
            .join_rev(
                JoinType::Join,
                contracts::Entity::belongs_to(code::Entity)
                    .from(contracts::Column::CreationCodeHash)
                    .to(code::Column::CodeHash)
                    .into(),
            )
            .filter(contracts::Column::Id.eq(vec![1u8, 2, 3, 4]))
            .build(DatabaseBackend::Postgres)
            .to_string();

        assert_eq!("", query);
    }
}
