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
    ContractDeployment, RetrieveContractDeployment, VerifiedContract, VerifiedContractMatches,
};

/************************ Public methods **************************/

use anyhow::{Context, Error};
use sea_orm::{DatabaseConnection, TransactionTrait};
use verifier_alliance_entity::contract_deployments;

pub async fn retrieve_contract_deployment(
    database_connection: &DatabaseConnection,
    contract_deployment: RetrieveContractDeployment,
) -> Result<Option<contract_deployments::Model>, Error> {
    internal::retrieve_contract_deployment(database_connection, contract_deployment).await
}

pub async fn insert_contract_deployment(
    database_connection: &DatabaseConnection,
    contract_deployment: ContractDeployment,
) -> Result<contract_deployments::Model, Error> {
    let transaction = database_connection
        .begin()
        .await
        .context("begin transaction failed")?;
    let model = internal::insert_contract_deployment(&transaction, contract_deployment).await?;
    transaction
        .commit()
        .await
        .context("commit transaction failed")?;
    Ok(model)
}
