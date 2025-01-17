#![allow(dead_code)]

// mod alliance_solidity_multi_part_batch_import;
// mod alliance_solidity_standard_json_batch_import;
// mod solidity_sources_verify_multi_part;
// mod solidity_sources_verify_standard_json;

mod routes;
mod test_cases;
mod types;

/************************************************/

use entity::{bytecodes, files, parts, sources};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use verifier_alliance_entity::{
    compiled_contracts, contract_deployments, contracts, verified_contracts,
};

#[async_trait::async_trait]
pub trait VerifierAllianceDatabaseChecker {
    async fn check_contract(&self, db: &DatabaseConnection, contract: contracts::Model);

    async fn retrieve_contract_deployment(
        &self,
        db: &DatabaseConnection,
    ) -> Option<contract_deployments::Model>;
    async fn check_contract_deployment(
        &self,
        db: &DatabaseConnection,
    ) -> contract_deployments::Model;

    async fn retrieve_compiled_contract(
        &self,
        db: &DatabaseConnection,
    ) -> Option<compiled_contracts::Model>;

    async fn check_compiled_contract(&self, db: &DatabaseConnection) -> compiled_contracts::Model;

    async fn check_verified_contract(
        &self,
        db: &DatabaseConnection,
        contract_deployment: &contract_deployments::Model,
        compiled_contract: &compiled_contracts::Model,
    ) -> verified_contracts::Model;

    async fn retrieve_verified_contract(
        db: &DatabaseConnection,
        contract_deployment: Option<&contract_deployments::Model>,
        compiled_contract: Option<&compiled_contracts::Model>,
    ) -> Option<verified_contracts::Model> {
        let mut query = verified_contracts::Entity::find();
        if let Some(contract_deployment) = contract_deployment {
            query =
                query.filter(verified_contracts::Column::DeploymentId.eq(contract_deployment.id))
        }
        if let Some(compiled_contract) = compiled_contract {
            query = query.filter(verified_contracts::Column::CompilationId.eq(compiled_contract.id))
        }
        query
            .one(db)
            .await
            .expect("Error while retrieving verified contract")
    }
}

#[async_trait::async_trait]
pub trait EthBytecodeDbDatabaseChecker {
    async fn check_source(&self, db: &DatabaseConnection) -> sources::Model;

    async fn check_files(&self, db: &DatabaseConnection) -> Vec<files::Model>;

    async fn check_source_files(
        &self,
        db: &DatabaseConnection,
        source: &sources::Model,
        files: &[files::Model],
    );

    async fn check_bytecodes(
        &self,
        db: &DatabaseConnection,
        source: &sources::Model,
    ) -> Vec<bytecodes::Model>;

    async fn check_parts(&self, db: &DatabaseConnection) -> Vec<parts::Model>;

    async fn check_bytecode_parts(
        &self,
        db: &DatabaseConnection,
        bytecodes: &[bytecodes::Model],
        parts: &[parts::Model],
    );
}
