/// Provides access to internal functions to access the database.
/// They mostly do not provide transactions consistency, and require
/// users to be care of transactions themselves.
///
/// Are not recommended to be used directly.
/// Prefer methods exposed to the public instead.
pub mod internal;

mod helpers;
mod types;

pub use types::{
    CompiledContract, CompiledContractCompiler, CompiledContractLanguage, ContractCode,
    ContractDeployment, InsertContractDeployment, RetrieveContractDeployment,
    RetrievedVerifiedContract, VerifiedContract, VerifiedContractMatches,
};

/************************ Public methods **************************/

use anyhow::{anyhow, Context, Error};
use sea_orm::{DatabaseConnection, TransactionTrait};

pub async fn insert_contract_deployment(
    database_connection: &DatabaseConnection,
    to_insert: InsertContractDeployment,
) -> Result<ContractDeployment, Error> {
    let chain_id = to_insert.chain_id();
    let address = to_insert.address().to_owned();
    let creation_code = to_insert.creation_code().map(ToOwned::to_owned);
    let runtime_code = to_insert.runtime_code().to_owned();

    let transaction = database_connection
        .begin()
        .await
        .context("begin transaction")?;

    let internal_data = internal::InternalContractDeploymentData::from(to_insert);
    let contract_model =
        internal::insert_contract(&transaction, internal_data.contract_code.clone()).await?;
    let contract_deployment_model =
        internal::insert_contract_deployment(&transaction, internal_data, contract_model.id)
            .await?;

    transaction.commit().await.context("commit transaction")?;

    Ok(ContractDeployment {
        id: contract_deployment_model.id,
        chain_id,
        address,
        runtime_code,
        creation_code,
        model: contract_deployment_model,
    })
}

pub async fn insert_verified_contract(
    database_connection: &DatabaseConnection,
    mut verified_contract: VerifiedContract,
) -> Result<(), Error> {
    let transaction = database_connection
        .begin()
        .await
        .context("begin transaction")?;

    let sources = std::mem::take(&mut verified_contract.compiled_contract.sources);
    let source_hashes = internal::precalculate_source_hashes(&sources);

    let compiled_contract_model =
        internal::insert_compiled_contract(&transaction, verified_contract.compiled_contract)
            .await?;
    let compiled_contract_id = compiled_contract_model.id;

    let _source_models = internal::insert_sources(&transaction, sources).await?;
    let _compiled_contract_source_models = internal::insert_compiled_contract_sources(
        &transaction,
        source_hashes,
        compiled_contract_id,
    )
    .await?;
    let _verified_contract_model = internal::insert_verified_contract(
        &transaction,
        verified_contract.contract_deployment_id,
        compiled_contract_id,
        verified_contract.matches,
    )
    .await?;

    transaction.commit().await.context("commit transaction")?;

    Ok(())
}

pub async fn find_contract_deployment(
    database_connection: &DatabaseConnection,
    to_retrieve: RetrieveContractDeployment,
) -> Result<Option<ContractDeployment>, Error> {
    let chain_id = to_retrieve.chain_id();
    let address = to_retrieve.address().to_owned();

    let contract_deployment_model =
        internal::retrieve_contract_deployment(database_connection, to_retrieve).await?;
    if let Some(contract_deployment_model) = contract_deployment_model {
        let contract = internal::retrieve_contract_by_id(
            database_connection,
            contract_deployment_model.contract_id,
        )
        .await?;

        let creation_code_model =
            internal::retrieve_code_by_id(database_connection, contract.creation_code_hash.clone())
                .await?;
        let creation_code = creation_code_model.code;

        let runtime_code_model =
            internal::retrieve_code_by_id(database_connection, contract.runtime_code_hash.clone())
                .await?;
        let runtime_code = runtime_code_model
            .code
            .ok_or(anyhow!("contract does not have runtime code"))?;

        return Ok(Some(ContractDeployment {
            id: contract_deployment_model.id,
            chain_id,
            address,
            runtime_code,
            creation_code,
            model: contract_deployment_model,
        }));
    }

    Ok(None)
}

pub async fn find_verified_contracts(
    database_connection: &DatabaseConnection,
    chain_id: u128,
    contract_address: Vec<u8>,
) -> Result<Vec<RetrievedVerifiedContract>, Error> {
    let database_connection = database_connection
        .begin()
        .await
        .context("begin transaction")?;
    let mut contract_deployment_models =
        internal::retrieve_contract_deployments_by_chain_id_and_address(
            &database_connection,
            chain_id,
            contract_address,
        )
        .await?;
    contract_deployment_models.sort_by_key(|model| model.updated_at);

    let mut verified_contracts = Vec::new();
    if let Some(contract_deployment_model) = contract_deployment_models.pop() {
        let contract_deployment_id = contract_deployment_model.id;
        let verified_contract_models = internal::retrieve_verified_contracts_by_deployment_id(
            &database_connection,
            contract_deployment_id,
        )
        .await?;
        for verified_contract_model in verified_contract_models {
            let compiled_contract_model = internal::retrieve_compiled_contract_by_id(&database_connection, verified_contract_model.compilation_id)
                .await?
                .ok_or_else(|| anyhow!("compiled contract does not exist in the database; verified_contracts.id={}, compiled_contracts.id={}", verified_contract_model.id, verified_contract_model.compilation_id))?;

            let creation_code_model = internal::retrieve_code_by_id(
                &database_connection,
                compiled_contract_model.creation_code_hash.clone(),
            )
            .await?;
            let creation_code = creation_code_model
                .code
                .ok_or(anyhow!("compiled contract does not have creation code"))?;

            let runtime_code_model = internal::retrieve_code_by_id(
                &database_connection,
                compiled_contract_model.runtime_code_hash.clone(),
            )
            .await?;
            let runtime_code = runtime_code_model
                .code
                .ok_or(anyhow!("compiled contract does not have runtime code"))?;

            let created_at = verified_contract_model.created_at;
            let updated_at = verified_contract_model.updated_at;
            let created_by = verified_contract_model.created_by.clone();
            let updated_by = verified_contract_model.updated_by.clone();

            let sources = internal::retrieve_sources_by_compilation_id(
                &database_connection,
                compiled_contract_model.id,
            )
            .await?;

            let verified_contract = internal::try_models_into_verified_contract(
                contract_deployment_id,
                compiled_contract_model,
                creation_code,
                runtime_code,
                sources,
                verified_contract_model,
            )?;

            verified_contracts.push(RetrievedVerifiedContract {
                verified_contract,
                created_at,
                updated_at,
                created_by,
                updated_by,
            });
        }
    }

    // At the end of the function
    database_connection
        .rollback()
        .await
        .context("rollback transaction")?;

    Ok(verified_contracts)
}
