use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use sha3::{Digest, Keccak256};
use std::{collections::BTreeMap, str::FromStr};
use verification_common::verifier_alliance::{
    CompilationArtifacts, CreationCodeArtifacts, MatchTransformation, MatchValues,
    RuntimeCodeArtifacts,
};
use verifier_alliance_entity_v1::{
    code, compiled_contracts, compiled_contracts_sources, contract_deployments, contracts, sources,
    verified_contracts,
};

#[serde_with::serde_as]
#[derive(Clone, Debug, Deserialize)]
pub struct TestCase {
    #[serde(default)]
    pub test_case_name: String,
    #[serde(deserialize_with = "string_to_u128")]
    pub chain_id: u128,
    #[serde_as(as = "blockscout_display_bytes::serde_as::Hex")]
    pub address: Vec<u8>,
    #[serde_as(as = "blockscout_display_bytes::serde_as::Hex")]
    pub transaction_hash: Vec<u8>,
    #[serde(deserialize_with = "string_to_u128")]
    pub block_number: u128,
    #[serde(deserialize_with = "string_to_u128")]
    pub transaction_index: u128,
    #[serde_as(as = "blockscout_display_bytes::serde_as::Hex")]
    pub deployer: Vec<u8>,

    #[serde_as(as = "blockscout_display_bytes::serde_as::Hex")]
    pub deployed_creation_code: Vec<u8>,
    #[serde_as(as = "blockscout_display_bytes::serde_as::Hex")]
    pub deployed_runtime_code: Vec<u8>,

    #[serde_as(as = "blockscout_display_bytes::serde_as::Hex")]
    pub compiled_creation_code: Vec<u8>,
    #[serde_as(as = "blockscout_display_bytes::serde_as::Hex")]
    pub compiled_runtime_code: Vec<u8>,

    pub compiler: String,
    pub version: String,
    pub language: String,
    pub name: String,
    pub fully_qualified_name: String,
    pub sources: BTreeMap<String, String>,
    pub compiler_settings: Value,
    pub compilation_artifacts: CompilationArtifacts,
    pub creation_code_artifacts: CreationCodeArtifacts,
    pub runtime_code_artifacts: RuntimeCodeArtifacts,

    pub creation_match: bool,
    pub creation_metadata_match: bool,
    pub creation_values: MatchValues,
    pub creation_transformations: Vec<MatchTransformation>,

    pub runtime_match: bool,
    pub runtime_metadata_match: bool,
    pub runtime_values: MatchValues,
    pub runtime_transformations: Vec<MatchTransformation>,
}

fn string_to_u128<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    u128::from_str(&string).map_err(<D::Error as serde::de::Error>::custom)
}

impl TestCase {
    pub fn from_content(name: impl Into<String>, content: &str) -> Self {
        let mut test_case: Self = serde_json::from_str(content).expect("invalid test case format");
        test_case.test_case_name = name.into();
        test_case
    }
}

impl TestCase {
    pub async fn validate_final_database_state(&self, database_connection: &DatabaseConnection) {
        let _contract_deployment = self
            .validate_contract_deployments_table(database_connection)
            .await;
        let contract = self.validate_contracts_table(database_connection).await;
        self.validate_code_value(
            database_connection,
            contract.creation_code_hash,
            self.deployed_creation_code.clone(),
        )
        .await;
        self.validate_code_value(
            database_connection,
            contract.runtime_code_hash,
            self.deployed_runtime_code.clone(),
        )
        .await;

        let compiled_contract = self
            .validate_compiled_contracts_table(database_connection)
            .await;
        self.validate_code_value(
            database_connection,
            compiled_contract.creation_code_hash,
            self.compiled_creation_code.clone(),
        )
        .await;
        self.validate_code_value(
            database_connection,
            compiled_contract.runtime_code_hash,
            self.compiled_runtime_code.clone(),
        )
        .await;

        let sources = self.validate_sources_table(database_connection).await;
        let _compiled_contracts_sources = self
            .validate_compiled_contracts_sources_table(database_connection, sources)
            .await;

        let _verified_contracts = self
            .validate_verified_contracts_table(database_connection)
            .await;
    }

    async fn validate_contract_deployments_table(
        &self,
        database_connection: &DatabaseConnection,
    ) -> contract_deployments::Model {
        let contract_deployments = contract_deployments::Entity::find()
            .all(database_connection)
            .await
            .expect("error while retrieving contract deployments");
        assert_eq!(
            contract_deployments.len(),
            1,
            "invalid number of contract deployments in the database: {:?}",
            contract_deployments
        );
        let contract_deployment = contract_deployments[0].clone();

        assert_eq!(
            contract_deployment.address,
            self.address.clone(),
            "invalid contract deployment address"
        );
        assert_eq!(
            contract_deployment.chain_id,
            self.chain_id.into(),
            "invalid contract deployment chain id"
        );
        assert_eq!(
            contract_deployment.transaction_hash, self.transaction_hash,
            "invalid contract deployment transaction hash"
        );
        assert_eq!(
            contract_deployment.block_number,
            self.block_number.into(),
            "invalid contract deployment block number"
        );
        assert_eq!(
            contract_deployment.transaction_index,
            self.transaction_index.into(),
            "invalid contract deployment transaction index"
        );
        assert_eq!(
            contract_deployment.deployer, self.deployer,
            "invalid contract deployment deployer"
        );

        contract_deployment
    }

    async fn validate_contracts_table(
        &self,
        database_connection: &DatabaseConnection,
    ) -> contracts::Model {
        let contracts = contracts::Entity::find()
            .all(database_connection)
            .await
            .expect("error while retrieving contracts");
        assert_eq!(
            contracts.len(),
            1,
            "invalid number of contracts in the database: {:?}",
            contracts
        );
        contracts[0].clone()
    }

    async fn validate_code_value(
        &self,
        database_connection: &DatabaseConnection,
        code_hash: Vec<u8>,
        code: Vec<u8>,
    ) -> code::Model {
        let code_model = code::Entity::find()
            .filter(code::Column::CodeHash.eq(code_hash.clone()))
            .one(database_connection)
            .await
            .expect("error while retrieving code value")
            .unwrap_or_else(|| panic!("code hash does not exist in the database: {:?}", code_hash));

        let expected_code_hash_keccak = Keccak256::digest(&code).to_vec();
        assert_eq!(
            code_model.code_hash_keccak, expected_code_hash_keccak,
            "invalid code code hash keccak"
        );
        assert_eq!(code_model.code, Some(code), "invalid code value");

        code_model
    }

    async fn validate_compiled_contracts_table(
        &self,
        database_connection: &DatabaseConnection,
    ) -> compiled_contracts::Model {
        let compiled_contracts = compiled_contracts::Entity::find()
            .all(database_connection)
            .await
            .expect("error while retrieving compiled contracts");
        assert_eq!(
            compiled_contracts.len(),
            1,
            "invalid number of compiled contracts in the database: {:?}",
            compiled_contracts
        );
        let compiled_contract = compiled_contracts[0].clone();

        assert_eq!(
            compiled_contract.compiler.to_string(),
            self.compiler,
            "invalid compiled contract compiler"
        );
        assert_eq!(
            compiled_contract.version, self.version,
            "invalid compiled contract version"
        );
        assert_eq!(
            compiled_contract.language.to_string(),
            self.language,
            "invalid compiled contract language"
        );
        assert_eq!(
            compiled_contract.name, self.name,
            "invalid compiled contract name"
        );
        assert_eq!(
            compiled_contract.fully_qualified_name, self.fully_qualified_name,
            "invalid compiled contract fully qualified name"
        );
        assert_eq!(
            compiled_contract.compiler_settings, self.compiler_settings,
            "invalid compiled contract compiler settings"
        );
        assert_eq!(
            compiled_contract.compilation_artifacts,
            Value::from(self.compilation_artifacts.clone()),
            "invalid compiled contract compilation artifacts"
        );
        assert_eq!(
            compiled_contract.creation_code_artifacts,
            Value::from(self.creation_code_artifacts.clone()),
            "invalid compiled contract creation code artifacts"
        );
        assert_eq!(
            compiled_contract.runtime_code_artifacts,
            Value::from(self.runtime_code_artifacts.clone()),
            "invalid compiled contract runtime artifacts"
        );

        compiled_contract
    }

    async fn validate_sources_table(
        &self,
        database_connection: &DatabaseConnection,
    ) -> Vec<sources::Model> {
        let sources = sources::Entity::find()
            .all(database_connection)
            .await
            .expect("error while retrieving sources");
        assert_eq!(
            sources.len(),
            self.sources.len(),
            "invalid number of sources in database: {:?}",
            sources
        );

        for (path, content) in self.sources.iter() {
            let source = sources
                .iter()
                .find(|source| &source.content == content)
                .unwrap_or_else(|| panic!("source not found in the database for path={path}"));
            let expected_source_hash_keccak = Keccak256::digest(&source.content).to_vec();
            assert_eq!(
                source.source_hash_keccak, expected_source_hash_keccak,
                "invalid source source hash keccak"
            );
        }

        sources
    }

    async fn validate_compiled_contracts_sources_table(
        &self,
        database_connection: &DatabaseConnection,
        sources: Vec<sources::Model>,
    ) -> Vec<compiled_contracts_sources::Model> {
        let compiled_contracts_sources = compiled_contracts_sources::Entity::find()
            .all(database_connection)
            .await
            .expect("error while retrieving compiled contracts sources");
        assert_eq!(
            compiled_contracts_sources.len(),
            sources.len(),
            "invalid number of compiled contracts sources in the database: {:?}",
            compiled_contracts_sources
        );

        for (path, content) in self.sources.iter() {
            let compiled_contract_source = compiled_contracts_sources
                .iter()
                .find(|model| &model.path == path)
                .unwrap_or_else(|| {
                    panic!("compiled contract source not found in database for path={path}")
                });
            let source = sources
                .iter()
                .find(|source| &source.content == content)
                .unwrap();
            let expected_source_hash = &source.source_hash;
            assert_eq!(
                &compiled_contract_source.source_hash, expected_source_hash,
                "invalid compiled contract source source hash"
            );
        }

        compiled_contracts_sources
    }

    async fn validate_verified_contracts_table(
        &self,
        database_connection: &DatabaseConnection,
    ) -> verified_contracts::Model {
        let verified_contracts = verified_contracts::Entity::find()
            .all(database_connection)
            .await
            .expect("error while retrieving verified contracts");
        assert_eq!(
            verified_contracts.len(),
            1,
            "invalid number of verified contracts in database: {:?}",
            verified_contracts
        );
        let verified_contract = verified_contracts[0].clone();

        assert!(
            verified_contract.creation_match,
            "invalid verified contract creation match"
        );
        assert_eq!(
            verified_contract.creation_metadata_match,
            Some(self.creation_metadata_match),
            "invalid verified contract creation metadata match"
        );
        assert_eq!(
            verified_contract.creation_values,
            Some(Value::from(self.creation_values.clone())),
            "invalid verified contract creation values"
        );
        assert_eq!(
            verified_contract.creation_transformations,
            Some(Value::from(self.creation_transformations.clone())),
            "invalid verified contract creation transformations"
        );

        assert!(
            verified_contract.runtime_match,
            "invalid verified contract runtime match"
        );
        assert_eq!(
            verified_contract.runtime_metadata_match,
            Some(self.runtime_metadata_match),
            "invalid verified contract runtime metadata match"
        );
        assert_eq!(
            verified_contract.runtime_values,
            Some(Value::from(self.runtime_values.clone())),
            "invalid verified contract runtime values"
        );
        assert_eq!(
            verified_contract.runtime_transformations,
            Some(Value::from(self.runtime_transformations.clone())),
            "invalid verified contract runtime transformations"
        );

        verified_contract
    }
}
