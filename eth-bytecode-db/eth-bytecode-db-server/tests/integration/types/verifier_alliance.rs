use crate::VerifierAllianceDatabaseChecker;
use blockscout_display_bytes::Bytes as DisplayBytes;
use pretty_assertions::assert_eq;
use sea_orm::{prelude::Decimal, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Deserialize;
use std::{collections::BTreeMap, str::FromStr, sync::Arc};
use verifier_alliance_entity::{
    code, compiled_contracts, contract_deployments, contracts, verified_contracts,
};

#[derive(Debug, Clone, Deserialize)]
pub struct TestCase {
    pub deployed_creation_code: Option<DisplayBytes>,
    pub deployed_runtime_code: DisplayBytes,

    pub compiled_creation_code: DisplayBytes,
    pub compiled_runtime_code: DisplayBytes,
    pub compiler: String,
    pub version: String,
    pub language: String,
    pub name: String,
    pub fully_qualified_name: String,
    pub sources: BTreeMap<String, String>,
    pub compiler_settings: serde_json::Value,
    pub compilation_artifacts: serde_json::Value,
    pub creation_code_artifacts: serde_json::Value,
    pub runtime_code_artifacts: serde_json::Value,

    pub creation_match: bool,
    pub creation_values: Option<serde_json::Value>,
    pub creation_transformations: Option<serde_json::Value>,

    pub runtime_match: bool,
    pub runtime_values: Option<serde_json::Value>,
    pub runtime_transformations: Option<serde_json::Value>,

    #[serde(default = "default_chain_id")]
    pub chain_id: usize,
    #[serde(default = "default_address")]
    pub address: DisplayBytes,
    #[serde(default = "default_transaction_hash")]
    pub transaction_hash: DisplayBytes,
    #[serde(default = "default_block_number")]
    pub block_number: i64,
    #[serde(default = "default_transaction_index")]
    pub transaction_index: i64,
    #[serde(default = "default_deployer")]
    pub deployer: DisplayBytes,

    #[serde(default)]
    pub is_genesis: bool,
}

fn default_chain_id() -> usize {
    5
}
fn default_address() -> DisplayBytes {
    DisplayBytes::from_str("0xcafecafecafecafecafecafecafecafecafecafe").unwrap()
}
fn default_transaction_hash() -> DisplayBytes {
    DisplayBytes::from_str("0xcafecafecafecafecafecafecafecafecafecafecafecafecafecafecafecafe")
        .unwrap()
}
fn default_block_number() -> i64 {
    1
}
fn default_transaction_index() -> i64 {
    0
}
fn default_deployer() -> DisplayBytes {
    DisplayBytes::from_str("0xfacefacefacefacefacefacefacefacefaceface").unwrap()
}

impl TestCase {
    pub fn standard_input(&self) -> serde_json::Value {
        let input = foundry_compilers::CompilerInput {
            language: self.language.clone(),
            sources: self
                .sources
                .iter()
                .map(|(file_path, content)| {
                    (
                        std::path::PathBuf::from(file_path),
                        foundry_compilers::artifacts::Source {
                            content: Arc::new(content.clone()),
                        },
                    )
                })
                .collect(),
            settings: serde_json::from_value(self.compiler_settings.clone())
                .expect("settings deserialization"),
        };

        serde_json::to_value(&input).unwrap()
    }

    pub fn contract_name(&self) -> String {
        self.fully_qualified_name
            .split(':')
            .last()
            .unwrap()
            .to_string()
    }

    pub fn file_name(&self) -> String {
        let name_parts: Vec<_> = self.fully_qualified_name.split(':').collect();
        name_parts
            .into_iter()
            .rev()
            .skip(1)
            .rev()
            .collect::<Vec<_>>()
            .join(":")
    }
}

#[async_trait::async_trait]
impl VerifierAllianceDatabaseChecker for TestCase {
    async fn check_contract(&self, db: &DatabaseConnection, contract: contracts::Model) {
        let creation_code = code::Entity::find_by_id(contract.creation_code_hash)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            self.deployed_creation_code.as_deref().map(Vec::from),
            creation_code.code,
            "Invalid creation_code for deployed contract"
        );

        let runtime_code = code::Entity::find_by_id(contract.runtime_code_hash)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            Some(self.deployed_runtime_code.to_vec()),
            runtime_code.code,
            "Invalid runtime_code for deployed contract"
        );
    }

    async fn retrieve_contract_deployment(
        &self,
        db: &DatabaseConnection,
    ) -> Option<contract_deployments::Model> {
        contract_deployments::Entity::find()
            .filter(contract_deployments::Column::ChainId.eq(Decimal::from(self.chain_id)))
            .filter(contract_deployments::Column::Address.eq(self.address.to_vec()))
            .filter(
                contract_deployments::Column::TransactionHash.eq(self.transaction_hash.to_vec()),
            )
            .one(db)
            .await
            .expect("Error while retrieving contract deployment")
    }

    async fn check_contract_deployment(
        &self,
        db: &DatabaseConnection,
    ) -> contract_deployments::Model {
        let contract_deployment = self
            .retrieve_contract_deployment(db)
            .await
            .expect("The data has not been added into `contract_deployments` table");

        let test_case_chain_id: Decimal = self.chain_id.into();
        let test_case_block_number: Decimal = self.block_number.into();
        let test_case_transaction_index: Decimal = self.transaction_index.into();
        assert_eq!(
            test_case_chain_id, contract_deployment.chain_id,
            "Invalid contract_deployments.chain_id"
        );
        assert_eq!(
            self.address.to_vec(),
            contract_deployment.address,
            "Invalid contract_deployments.address"
        );
        assert_eq!(
            self.transaction_hash.to_vec(),
            contract_deployment.transaction_hash,
            "Invalid contract_deployments.transaction_hash"
        );
        assert_eq!(
            test_case_block_number, contract_deployment.block_number,
            "Invalid contract_deployments.block_number"
        );
        assert_eq!(
            test_case_transaction_index, contract_deployment.transaction_index,
            "Invalid contract_deployments.transaction_index"
        );
        assert_eq!(
            self.deployer.to_vec(),
            contract_deployment.deployer,
            "Invalid contract_deployments.deployer"
        );

        let contract = contracts::Entity::find_by_id(contract_deployment.contract_id)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        self.check_contract(db, contract).await;

        contract_deployment
    }

    async fn retrieve_compiled_contract(
        &self,
        db: &DatabaseConnection,
    ) -> Option<compiled_contracts::Model> {
        let creation_code_hash = keccak_hash::keccak(&self.compiled_creation_code);
        let runtime_code_hash = keccak_hash::keccak(&self.compiled_runtime_code);
        compiled_contracts::Entity::find()
            .filter(compiled_contracts::Column::Compiler.eq(self.compiler.clone()))
            .filter(compiled_contracts::Column::Language.eq(self.language.clone()))
            .filter(compiled_contracts::Column::CreationCodeHash.eq(creation_code_hash.0.to_vec()))
            .filter(compiled_contracts::Column::RuntimeCodeHash.eq(runtime_code_hash.0.to_vec()))
            .one(db)
            .await
            .expect("Error while retrieving compiled contract")
    }

    async fn check_compiled_contract(&self, db: &DatabaseConnection) -> compiled_contracts::Model {
        let compiled_contract = self
            .retrieve_compiled_contract(db)
            .await
            .expect("The data has not been added into `compiled_contracts` table");

        let test_case_sources = serde_json::to_value(self.sources.clone()).unwrap();
        let test_case_creation_code_hash =
            keccak_hash::keccak(&self.compiled_creation_code).0.to_vec();
        let test_case_runtime_code_hash =
            keccak_hash::keccak(&self.compiled_runtime_code).0.to_vec();

        assert_eq!(
            self.compiler, compiled_contract.compiler,
            "Invalid compiler"
        );
        assert_eq!(self.version, compiled_contract.version, "Invalid version");
        assert_eq!(
            self.language, compiled_contract.language,
            "Invalid language"
        );
        assert_eq!(self.name, compiled_contract.name, "Invalid name");
        assert_eq!(
            self.fully_qualified_name, compiled_contract.fully_qualified_name,
            "Invalid fully_qualified_name"
        );
        assert_eq!(
            test_case_sources, compiled_contract.sources,
            "Invalid sources"
        );
        assert_eq!(
            self.compiler_settings, compiled_contract.compiler_settings,
            "Invalid compiler_settings"
        );
        assert_eq!(
            self.compilation_artifacts, compiled_contract.compilation_artifacts,
            "Invalid compilation_artifacts"
        );
        assert_eq!(
            test_case_creation_code_hash, compiled_contract.creation_code_hash,
            "Invalid creation_code_hash"
        );
        assert_eq!(
            self.creation_code_artifacts, compiled_contract.creation_code_artifacts,
            "Invalid creation_code_artifacts"
        );
        assert_eq!(
            test_case_runtime_code_hash, compiled_contract.runtime_code_hash,
            "Invalid runtime_code_hash"
        );
        assert_eq!(
            self.runtime_code_artifacts, compiled_contract.runtime_code_artifacts,
            "Invalid runtime_code_artifacts"
        );

        compiled_contract
    }

    async fn check_verified_contract(
        &self,
        db: &DatabaseConnection,
        contract_deployment: &contract_deployments::Model,
        compiled_contract: &compiled_contracts::Model,
    ) -> verified_contracts::Model {
        let verified_contract = Self::retrieve_verified_contract(
            db,
            Some(contract_deployment),
            Some(compiled_contract),
        )
        .await
        .expect("The data has not been added into `verified_contracts` table");

        assert_eq!(
            self.creation_match, verified_contract.creation_match,
            "Invalid creation_match"
        );
        assert_eq!(
            self.creation_values, verified_contract.creation_values,
            "Invalid creation_values"
        );
        assert_eq!(
            self.creation_transformations, verified_contract.creation_transformations,
            "Invalid creation_transformations"
        );
        assert_eq!(
            self.runtime_match, verified_contract.runtime_match,
            "Invalid runtime_match"
        );
        assert_eq!(
            self.runtime_values, verified_contract.runtime_values,
            "Invalid runtime_values"
        );
        assert_eq!(
            self.runtime_transformations, verified_contract.runtime_transformations,
            "Invalid runtime_transformations"
        );

        verified_contract
    }
}
