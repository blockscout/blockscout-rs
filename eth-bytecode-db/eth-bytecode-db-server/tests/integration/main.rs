mod types;
mod verifier_alliance;

/************************************************/

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use types::{TestCaseRequest, TestCaseResponse, TestCaseRoute};
use verifier_alliance_entity::{
    compiled_contracts, contract_deployments, contracts, verified_contracts,
};

pub trait VerifierServiceRequest<EthBytecodeDbRoute> {
    type VerifierRequest;

    fn with(&self, request: &tonic::Request<Self::VerifierRequest>) -> bool;
}

pub trait VerifierServiceResponse<EthBytecodeDbRoute> {
    type VerifierResponse;

    fn returning_const(&self) -> Self::VerifierResponse;
}

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
