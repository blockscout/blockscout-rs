use crate::{
    routes,
    types::artifacts::{
        LosslessCodeParts, LosslessCodeValues, LosslessCompilationArtifacts,
        LosslessCompilerSettings, LosslessCreationCodeArtifacts, LosslessRuntimeCodeArtifacts,
    },
    EthBytecodeDbDatabaseChecker, VerifierAllianceDatabaseChecker,
};
use blockscout_display_bytes::Bytes as DisplayBytes;
use entity::{
    bytecode_parts, bytecodes, files, parts, sea_orm_active_enums, source_files, sources,
};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use pretty_assertions::assert_eq;
use routes::{eth_bytecode_db, verifier};
use sea_orm::{prelude::Decimal, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Deserialize;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
    sync::Arc,
};
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
    pub compiler_settings: LosslessCompilerSettings,
    pub compilation_artifacts: LosslessCompilationArtifacts,
    pub creation_code_artifacts: LosslessCreationCodeArtifacts,
    pub runtime_code_artifacts: LosslessRuntimeCodeArtifacts,

    pub creation_match: bool,
    pub creation_values: Option<LosslessCodeValues>,
    pub creation_transformations: Option<serde_json::Value>,

    pub runtime_match: bool,
    pub runtime_values: Option<LosslessCodeValues>,
    pub runtime_transformations: Option<serde_json::Value>,

    pub creation_code_parts: Vec<LosslessCodeParts>,
    pub runtime_code_parts: Vec<LosslessCodeParts>,

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

pub fn default_chain_id() -> usize {
    5
}
pub fn default_address() -> DisplayBytes {
    DisplayBytes::from_str("0xcafecafecafecafecafecafecafecafecafecafe").unwrap()
}
pub fn default_transaction_hash() -> DisplayBytes {
    DisplayBytes::from_str("0xcafecafecafecafecafecafecafecafecafecafecafecafecafecafecafecafe")
        .unwrap()
}
pub fn default_block_number() -> i64 {
    1
}
pub fn default_transaction_index() -> i64 {
    0
}
pub fn default_deployer() -> DisplayBytes {
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
            settings: serde_json::from_value(self.compiler_settings.raw.clone())
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

    pub fn abi(&self) -> Option<&serde_json::Value> {
        self.compilation_artifacts.parsed.abi.as_ref()
    }

    pub fn constructor_arguments(&self) -> Option<String> {
        self.creation_values
            .as_ref()
            .and_then(|v| v.parsed.constructor_arguments.clone())
    }

    pub fn evm_version(&self) -> Option<String> {
        self.compiler_settings
            .parsed
            .evm_version
            .map(|v| v.to_string())
    }

    pub fn optimization_runs(&self) -> Option<u32> {
        self.compiler_settings
            .parsed
            .optimizer
            .enabled
            .unwrap_or_default()
            .then(|| {
                self.compiler_settings
                    .parsed
                    .optimizer
                    .runs
                    .map(|v| v as u32)
            })
            .flatten()
    }

    pub fn libraries(&self) -> BTreeMap<String, String> {
        #[allow(clippy::iter_overeager_cloned)]
        self.compiler_settings
            .parsed
            .libraries
            .libs
            .values()
            .cloned()
            .flatten()
            .collect()
    }

    pub fn verifier_alliance_contract(&self) -> eth_bytecode_db_v2::VerifierAllianceContract {
        eth_bytecode_db_v2::VerifierAllianceContract {
            chain_id: format!("{}", self.chain_id),
            contract_address: self.address.to_string(),
            transaction_hash: Some(self.transaction_hash.to_string()),
            block_number: Some(self.block_number),
            transaction_index: Some(self.transaction_index),
            deployer: Some(self.deployer.to_string()),
            creation_code: self.deployed_creation_code.as_ref().map(|v| v.to_string()),
            runtime_code: self.deployed_runtime_code.to_string(),
        }
    }

    pub fn verification_metadata(&self) -> eth_bytecode_db_v2::VerificationMetadata {
        let transaction_hash = (!self.is_genesis).then_some(self.transaction_hash.to_string());
        let block_number = (!self.is_genesis).then_some(self.block_number);
        let transaction_index = (!self.is_genesis).then_some(self.transaction_index);
        let deployer = (!self.is_genesis).then_some(self.deployer.to_string());

        eth_bytecode_db_v2::VerificationMetadata {
            chain_id: Some(format!("{}", self.chain_id)),
            contract_address: Some(self.address.to_string()),
            transaction_hash,
            block_number,
            transaction_index,
            deployer,
            creation_code: self
                .deployed_creation_code
                .as_ref()
                .map(ToString::to_string),
            runtime_code: Some(self.deployed_runtime_code.to_string()),
        }
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
            self.compiler_settings.raw, compiled_contract.compiler_settings,
            "Invalid compiler_settings"
        );
        assert_eq!(
            self.compilation_artifacts.raw, compiled_contract.compilation_artifacts,
            "Invalid compilation_artifacts"
        );
        assert_eq!(
            test_case_creation_code_hash, compiled_contract.creation_code_hash,
            "Invalid creation_code_hash"
        );
        assert_eq!(
            self.creation_code_artifacts.raw, compiled_contract.creation_code_artifacts,
            "Invalid creation_code_artifacts"
        );
        assert_eq!(
            test_case_runtime_code_hash, compiled_contract.runtime_code_hash,
            "Invalid runtime_code_hash"
        );
        assert_eq!(
            self.runtime_code_artifacts.raw, compiled_contract.runtime_code_artifacts,
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
            self.creation_values.as_ref().map(|v| &v.raw),
            verified_contract.creation_values.as_ref(),
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
            self.runtime_values.as_ref().map(|v| &v.raw),
            verified_contract.runtime_values.as_ref(),
            "Invalid runtime_values"
        );
        assert_eq!(
            self.runtime_transformations, verified_contract.runtime_transformations,
            "Invalid runtime_transformations"
        );

        verified_contract
    }
}

#[async_trait::async_trait]
impl EthBytecodeDbDatabaseChecker for TestCase {
    async fn check_source(&self, db: &DatabaseConnection) -> sources::Model {
        let sources = sources::Entity::find()
            .all(db)
            .await
            .expect("Error while reading source");
        assert_eq!(
            1,
            sources.len(),
            "Invalid number of sources returned. Expected 1, actual {}",
            sources.len()
        );
        let db_source = &sources[0];
        assert_eq!(
            self.language.to_lowercase(),
            db_source.source_type.to_string().to_lowercase(),
            "Invalid source type"
        );
        assert_eq!(
            self.version, db_source.compiler_version,
            "Invalid compiler version"
        );
        assert_eq!(
            self.compiler_settings.raw, db_source.compiler_settings,
            "Invalid compiler settings"
        );
        assert_eq!(self.file_name(), db_source.file_name, "Invalid file name");
        assert_eq!(
            self.contract_name(),
            db_source.contract_name,
            "Invalid contract name"
        );
        assert_eq!(self.abi(), db_source.abi.as_ref(), "Invalid abi");
        assert_eq!(
            Some(&self.compilation_artifacts.raw),
            db_source.compilation_artifacts.as_ref(),
            "Invalid compilation artifacts"
        );
        assert_eq!(
            Some(&self.creation_code_artifacts.raw),
            db_source.creation_input_artifacts.as_ref(),
            "Invalid creation input artifacts"
        );
        assert_eq!(
            Some(&self.runtime_code_artifacts.raw),
            db_source.deployed_bytecode_artifacts.as_ref(),
            "Invalid deployed bytecode artifacts"
        );

        assert_eq!(
            self.compiled_creation_code, db_source.raw_creation_input,
            "Invalid raw creation input"
        );
        assert_eq!(
            self.compiled_runtime_code, db_source.raw_deployed_bytecode,
            "Invalid raw deployed bytecode"
        );

        db_source.clone()
    }

    async fn check_files(&self, db: &DatabaseConnection) -> Vec<files::Model> {
        let files = files::Entity::find()
            .all(db)
            .await
            .expect("Error while reading files");
        let parsed_files = files
            .clone()
            .into_iter()
            .map(|v| (v.name, v.content))
            .collect();

        assert_eq!(self.sources, parsed_files, "Invalid source files");

        files
    }

    async fn check_source_files(
        &self,
        db: &DatabaseConnection,
        source: &sources::Model,
        files: &[files::Model],
    ) {
        let source_files = source_files::Entity::find()
            .all(db)
            .await
            .expect("Error while reading source files");
        assert!(
            source_files
                .iter()
                .all(|value| value.source_id == source.id),
            "Invalid source id in retrieved source files"
        );
        let expected_file_ids = files.iter().map(|file| file.id).collect::<HashSet<_>>();
        assert_eq!(
            expected_file_ids,
            source_files
                .iter()
                .map(|value| value.file_id)
                .collect::<HashSet<_>>(),
            "Invalid file ids in retrieved source files"
        );
    }

    async fn check_bytecodes(
        &self,
        db: &DatabaseConnection,
        source: &sources::Model,
    ) -> Vec<bytecodes::Model> {
        let bytecodes = bytecodes::Entity::find()
            .all(db)
            .await
            .expect("Error while reading bytecodes");
        assert!(
            bytecodes.iter().all(|value| value.source_id == source.id),
            "Invalid source id in retrieved bytecodes"
        );
        let expected_bytecode_types = [
            sea_orm_active_enums::BytecodeType::CreationInput,
            sea_orm_active_enums::BytecodeType::DeployedBytecode,
        ];
        assert!(
            expected_bytecode_types.iter().all(|expected| bytecodes
                .iter()
                .any(|bytecode| &bytecode.bytecode_type == expected)),
            "Invalid bytecode types in retrieved bytecodes"
        );

        bytecodes
    }

    async fn check_parts(&self, db: &DatabaseConnection) -> Vec<parts::Model> {
        let parts = parts::Entity::find()
            .all(db)
            .await
            .expect("Error while reading parts");

        let expected_main_parts_data: HashSet<_> = self
            .creation_code_parts
            .iter()
            .chain(&self.runtime_code_parts)
            .filter_map(|v| (v.parsed.r#type == "main").then_some(v.parsed.data.to_vec()))
            .collect();
        assert_eq!(
            expected_main_parts_data,
            parts
                .iter()
                .filter_map(
                    |part| (part.part_type == sea_orm_active_enums::PartType::Main)
                        .then_some(part.data.clone())
                )
                .collect::<HashSet<_>>(),
            "Invalid data returned for main parts"
        );
        let expected_meta_parts_data: HashSet<_> = self
            .creation_code_parts
            .iter()
            .chain(&self.runtime_code_parts)
            .filter_map(|v| (v.parsed.r#type == "meta").then_some(v.parsed.data.to_vec()))
            .collect();
        assert_eq!(
            expected_meta_parts_data,
            parts
                .iter()
                .filter_map(
                    |part| (part.part_type == sea_orm_active_enums::PartType::Metadata)
                        .then_some(part.data.clone())
                )
                .collect::<HashSet<_>>(),
            "Invalid data returned for meta parts"
        );

        parts
    }

    async fn check_bytecode_parts(
        &self,
        db: &DatabaseConnection,
        bytecodes: &[bytecodes::Model],
        parts: &[parts::Model],
    ) {
        let bytecode_parts = bytecode_parts::Entity::find()
            .all(db)
            .await
            .expect("Error while reading bytecode parts");

        let check_code_parts = |bytecode_type: sea_orm_active_enums::BytecodeType,
                                code_parts: &[LosslessCodeParts]| {
            let bytecode_id = bytecodes
                .iter()
                .find_map(|bytecode| {
                    (bytecode.bytecode_type == bytecode_type).then_some(bytecode.id)
                })
                .unwrap();
            let processed_parts: Vec<_> = code_parts
                .iter()
                .map(|v| {
                    parts
                        .iter()
                        .find(|part| {
                            let part_type = if v.parsed.r#type == "main" {
                                sea_orm_active_enums::PartType::Main
                            } else {
                                sea_orm_active_enums::PartType::Metadata
                            };
                            part.part_type == part_type && part.data == v.parsed.data
                        })
                        .unwrap()
                })
                .enumerate()
                .collect();
            let bytecode_parts: Vec<_> = bytecode_parts
                .iter()
                .filter(|bytecode_part| bytecode_part.bytecode_id == bytecode_id)
                .collect();
            assert_eq!(
                processed_parts.len(),
                bytecode_parts.len(),
                "Parts and bytecode parts length mismatch"
            );
            assert!(
                processed_parts
                    .iter()
                    .zip(bytecode_parts)
                    .all(|(part, bytecode_part)| {
                        bytecode_part.order as usize == part.0 && bytecode_part.part_id == part.1.id
                    }),
                "Invalid bytecode parts"
            );
        };

        check_code_parts(
            sea_orm_active_enums::BytecodeType::CreationInput,
            &self.creation_code_parts,
        );
        check_code_parts(
            sea_orm_active_enums::BytecodeType::DeployedBytecode,
            &self.runtime_code_parts,
        );
    }
}

mod responses {
    use super::*;
    use crate::types::VerifierResponse;

    impl VerifierResponse<smart_contract_verifier_v2::VerifyResponse> for TestCase {
        fn returning_const(&self) -> smart_contract_verifier_v2::VerifyResponse {
            let source_type = match self.language.to_lowercase().as_str() {
                "solidity" => smart_contract_verifier_v2::source::SourceType::Solidity,
                "yul" => smart_contract_verifier_v2::source::SourceType::Yul,
                "vyper" => smart_contract_verifier_v2::source::SourceType::Vyper,
                _ => panic!("unexpected language"),
            };

            smart_contract_verifier_v2::VerifyResponse {
                message: "Ok".to_string(),
                status: smart_contract_verifier_v2::verify_response::Status::Success.into(),
                source: Some(smart_contract_verifier_v2::Source {
                    file_name: self.file_name().clone(),
                    contract_name: self.contract_name().clone(),
                    compiler_version: self.version.clone(),
                    compiler_settings: self.compiler_settings.to_string(),
                    source_type: source_type.into(),
                    source_files: self.sources.clone(),
                    abi: self.abi().map(|v| v.to_string()),
                    constructor_arguments: self.constructor_arguments(),
                    match_type: 0,
                    compilation_artifacts: Some(self.compilation_artifacts.to_string()),
                    creation_input_artifacts: Some(self.creation_code_artifacts.to_string()),
                    deployed_bytecode_artifacts: Some(self.runtime_code_artifacts.to_string()),
                    is_blueprint: false,
                }),
                extra_data: Some(smart_contract_verifier_v2::verify_response::ExtraData {
                    local_creation_input_parts: self
                        .creation_code_parts
                        .iter()
                        .map(|v| serde_json::from_value(v.raw.clone()).unwrap())
                        .collect(),
                    local_deployed_bytecode_parts: self
                        .runtime_code_parts
                        .iter()
                        .map(|v| serde_json::from_value(v.raw.clone()).unwrap())
                        .collect(),
                }),
                post_action_responses: None,
            }
        }
    }

    impl VerifierResponse<smart_contract_verifier_v2::BatchVerifyResponse> for TestCase {
        fn returning_const(&self) -> smart_contract_verifier_v2::BatchVerifyResponse {
            let compiler = match self.compiler.to_lowercase().as_str() {
                "solc" => smart_contract_verifier_v2::contract_verification_success::compiler::Compiler::Solc,
                "vyper" => smart_contract_verifier_v2::contract_verification_success::compiler::Compiler::Vyper,
                _ => panic!("unexpected compiler")
            };
            let language = match self.language.to_lowercase().as_str() {
                "solidity" => smart_contract_verifier_v2::contract_verification_success::language::Language::Solidity,
                "yul" => smart_contract_verifier_v2::contract_verification_success::language::Language::Yul,
                "vyper" => smart_contract_verifier_v2::contract_verification_success::language::Language::Vyper,
                _ => panic!("unexpected language")
            };
            smart_contract_verifier_v2::BatchVerifyResponse {
                verification_result: Some(smart_contract_verifier_v2::batch_verify_response::VerificationResult::ContractVerificationResults(
                    smart_contract_verifier_v2::batch_verify_response::ContractVerificationResults {
                        items: vec![
                            smart_contract_verifier_v2::ContractVerificationResult {
                                verification_result: Some(smart_contract_verifier_v2::contract_verification_result::VerificationResult::Success(
                                    smart_contract_verifier_v2::ContractVerificationSuccess {
                                        creation_code: self.compiled_creation_code.to_string(),
                                        runtime_code: self.compiled_runtime_code.to_string(),
                                        compiler: compiler.into(),
                                        compiler_version: self.version.clone(),
                                        language: language.into(),
                                        file_name: self.file_name(),
                                        contract_name: self.contract_name(),
                                        sources: self.sources.clone(),
                                        compiler_settings: self.compiler_settings.to_string(),
                                        compilation_artifacts: self.compilation_artifacts.to_string(),
                                        creation_code_artifacts: self.creation_code_artifacts.to_string(),
                                        runtime_code_artifacts: self.runtime_code_artifacts.to_string(),
                                        creation_match_details: self.creation_match.then(|| {
                                            smart_contract_verifier_v2::contract_verification_success::MatchDetails {
                                                match_type: smart_contract_verifier_v2::contract_verification_success::MatchType::Undefined.into(),
                                                values: self.creation_values.as_ref().unwrap().to_string(),
                                                transformations: self.creation_transformations.as_ref().unwrap().to_string(),
                                            }
                                        }),
                                        runtime_match_details: self.runtime_match.then(|| {
                                            smart_contract_verifier_v2::contract_verification_success::MatchDetails {
                                                match_type: smart_contract_verifier_v2::contract_verification_success::MatchType::Undefined.into(),
                                                values: self.runtime_values.as_ref().unwrap().to_string(),
                                                transformations: self.runtime_transformations.as_ref().unwrap().to_string(),
                                            }
                                        }),
                                    }
                                )),
                            }
                        ],
                    }
                )),
            }
        }
    }
}

mod solidity_multi_part {
    use super::*;
    use crate::types::{Request, Route, VerifierRequest, VerifierRoute};

    impl Request<eth_bytecode_db::SoliditySourcesVerifyMultiPart> for TestCase {
        fn to_request(
            &self,
        ) -> <eth_bytecode_db::SoliditySourcesVerifyMultiPart as Route>::Request {
            eth_bytecode_db_v2::VerifySolidityMultiPartRequest {
                bytecode: self.deployed_runtime_code.to_string(),
                bytecode_type: eth_bytecode_db_v2::BytecodeType::DeployedBytecode.into(),
                compiler_version: self.version.clone(),
                evm_version: self.evm_version(),
                optimization_runs: self.optimization_runs().map(|v| v as i32),
                source_files: self.sources.clone(),
                libraries: self.libraries(),
                metadata: Some(self.verification_metadata()),
            }
        }
    }

    impl VerifierRequest<<verifier::SoliditySourcesVerifyMultiPart as VerifierRoute>::Request>
        for TestCase
    {
        fn with(
            &self,
            request: &tonic::Request<
                <verifier::SoliditySourcesVerifyMultiPart as VerifierRoute>::Request,
            >,
        ) -> bool {
            let request = &request.get_ref();

            request.compiler_version == self.version
                && request.evm_version == self.evm_version()
                && request.optimization_runs.map(|v| v as u32) == self.optimization_runs()
                && request.libraries == self.libraries()
        }
    }
}

mod solidity_standard_json {
    use super::*;
    use crate::types::{Request, Route, VerifierRequest, VerifierRoute};

    impl Request<eth_bytecode_db::SoliditySourcesVerifyStandardJson> for TestCase {
        fn to_request(
            &self,
        ) -> <eth_bytecode_db::SoliditySourcesVerifyStandardJson as Route>::Request {
            eth_bytecode_db_v2::VerifySolidityStandardJsonRequest {
                bytecode: self.deployed_runtime_code.to_string(),
                bytecode_type: eth_bytecode_db_v2::BytecodeType::DeployedBytecode.into(),
                compiler_version: self.version.clone(),
                input: self.standard_input().to_string(),
                metadata: Some(self.verification_metadata()),
            }
        }
    }

    impl VerifierRequest<<verifier::SoliditySourcesVerifyStandardJson as VerifierRoute>::Request>
        for TestCase
    {
        fn with(
            &self,
            request: &tonic::Request<
                <verifier::SoliditySourcesVerifyStandardJson as VerifierRoute>::Request,
            >,
        ) -> bool {
            let request = &request.get_ref();

            let input = self.standard_input().to_string();
            request.compiler_version == self.version && request.input == input
        }
    }
}

mod batch_import_solidity_multi_part {
    use super::*;
    use crate::{
        routes::{
            eth_bytecode_db::AllianceSolidityMultiPartBatchImport,
            verifier::SoliditySourcesBatchVerifyMultiPart,
        },
        types::{Request, Route, VerifierRequest, VerifierRoute},
    };

    impl Request<AllianceSolidityMultiPartBatchImport> for TestCase {
        fn to_request(&self) -> <AllianceSolidityMultiPartBatchImport as Route>::Request {
            eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityMultiPartRequest {
                contracts: vec![self.verifier_alliance_contract()],
                compiler_version: self.version.clone(),
                evm_version: self.evm_version(),
                optimization_runs: self.optimization_runs(),
                source_files: self.sources.clone(),
                libraries: self.libraries(),
            }
        }
    }

    impl VerifierRequest<<SoliditySourcesBatchVerifyMultiPart as VerifierRoute>::Request> for TestCase {
        fn with(
            &self,
            request: &tonic::Request<
                <SoliditySourcesBatchVerifyMultiPart as VerifierRoute>::Request,
            >,
        ) -> bool {
            let request = &request.get_ref();

            request.compiler_version == self.version
                && request.evm_version == self.evm_version()
                && request.optimization_runs == self.optimization_runs()
                && request.libraries == self.libraries()
        }
    }
}

mod batch_import_solidity_standard_json {
    use super::*;
    use crate::{
        routes::{
            eth_bytecode_db::AllianceSolidityStandardJsonBatchImport,
            verifier::SoliditySourcesBatchVerifyStandardJson,
        },
        types::{Request, Route, VerifierRequest, VerifierRoute},
    };

    impl Request<AllianceSolidityStandardJsonBatchImport> for TestCase {
        fn to_request(&self) -> <AllianceSolidityStandardJsonBatchImport as Route>::Request {
            eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityStandardJsonRequest {
                contracts: vec![self.verifier_alliance_contract()],
                compiler_version: self.version.clone(),
                input: self.standard_input().to_string(),
            }
        }
    }

    impl VerifierRequest<<SoliditySourcesBatchVerifyStandardJson as VerifierRoute>::Request>
        for TestCase
    {
        fn with(
            &self,
            request: &tonic::Request<
                <SoliditySourcesBatchVerifyStandardJson as VerifierRoute>::Request,
            >,
        ) -> bool {
            let request = &request.get_ref();

            let input = self.standard_input().to_string();
            request.compiler_version == self.version && request.input == input
        }
    }
}
