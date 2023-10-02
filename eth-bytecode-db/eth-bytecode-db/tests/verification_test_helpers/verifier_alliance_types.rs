use super::test_input_data::TestInputData;
use blockscout_display_bytes::Bytes as DisplayBytes;
use bytes::Bytes;
use eth_bytecode_db::verification::{BytecodeType, VerificationMetadata, VerificationRequest};
use serde::Deserialize;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use std::{collections::BTreeMap, path::Path, str::FromStr};

#[derive(Debug, Clone, Deserialize)]
pub struct TestCase {
    pub deployed_creation_code: DisplayBytes,
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
    pub creation_values: serde_json::Value,
    pub creation_transformations: serde_json::Value,

    pub runtime_match: bool,
    pub runtime_values: serde_json::Value,
    pub runtime_transformations: serde_json::Value,

    #[serde(skip_deserializing)]
    #[serde(default = "default_chain_id")]
    pub chain_id: usize,
    #[serde(skip_deserializing)]
    #[serde(default = "default_address")]
    pub address: DisplayBytes,
    #[serde(skip_deserializing)]
    #[serde(default = "default_transaction_hash")]
    pub transaction_hash: DisplayBytes,
    #[serde(skip_deserializing)]
    #[serde(default = "default_block_number")]
    pub block_number: usize,
    #[serde(skip_deserializing)]
    #[serde(default = "default_transaction_index")]
    pub transaction_index: usize,
    #[serde(skip_deserializing)]
    #[serde(default = "default_deployer")]
    pub deployer: DisplayBytes,
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
fn default_block_number() -> usize {
    1
}
fn default_transaction_index() -> usize {
    0
}
fn default_deployer() -> DisplayBytes {
    DisplayBytes::from_str("0xfacefacefacefacefacefacefacefacefaceface").unwrap()
}

impl TestCase {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        let content = std::fs::read_to_string(path).expect("failed to read file");
        serde_json::from_str(&content).expect("invalid test case format")
    }

    pub fn to_test_input_data<C>(
        &self,
        content: C,
        is_authorized: bool,
    ) -> TestInputData<VerificationRequest<C>> {
        let eth_bytecode_db_request = self.to_eth_bytecode_db_request(content, is_authorized);
        let verifier_source = self.to_verifier_source();
        let verifier_extra_data = self.to_verifier_extra_data();

        TestInputData::from_verifier_source_and_extra_data(
            eth_bytecode_db_request,
            verifier_source,
            verifier_extra_data,
        )
    }

    fn to_eth_bytecode_db_request<C>(
        &self,
        content: C,
        is_authorized: bool,
    ) -> VerificationRequest<C> {
        VerificationRequest {
            bytecode: self.deployed_creation_code.to_string(),
            bytecode_type: BytecodeType::CreationInput,
            compiler_version: self.version.clone(),
            content,
            metadata: Some(VerificationMetadata {
                chain_id: Some(self.chain_id as i64),
                contract_address: Some(self.address.0.clone()),
                transaction_hash: Some(self.transaction_hash.0.clone()),
                block_number: Some(self.block_number as i64),
                transaction_index: Some(self.transaction_index as i64),
                deployer: Some(self.deployer.0.clone()),
                creation_code: Some(self.deployed_creation_code.0.clone()),
                runtime_code: Some(self.deployed_runtime_code.0.clone()),
            }),
            is_authorized,
        }
    }

    fn to_verifier_source(&self) -> smart_contract_verifier_v2::Source {
        let file_name = self
            .fully_qualified_name
            .split(':')
            .next()
            .expect("':' should be used as a separator in the `fully_qualified_name`");
        let source_type = match self.language.as_str() {
            "solidity" => smart_contract_verifier_v2::source::SourceType::Solidity,
            "yul" => smart_contract_verifier_v2::source::SourceType::Yul,
            "vyper" => smart_contract_verifier_v2::source::SourceType::Vyper,
            language => panic!("Invalid language: {language}"),
        };
        let abi = self
            .compilation_artifacts
            .as_object()
            .expect("`compilation_artifacts` must be an object")
            .get("abi")
            .map(|abi| abi.to_string());
        let constructor_arguments = self
            .creation_values
            .as_object()
            .expect("`creation_values` must be an object")
            .get("constructorArguments")
            .map(|args| {
                args.as_str()
                    .expect("`constructorArguments` must be a string")
                    .to_string()
            });
        let match_type = if self
            .creation_values
            .as_object()
            .unwrap()
            .get("cborAuxdata")
            .is_some()
        {
            smart_contract_verifier_v2::source::MatchType::Partial
        } else {
            smart_contract_verifier_v2::source::MatchType::Full
        };

        smart_contract_verifier_v2::Source {
            file_name: file_name.to_string(),
            contract_name: self.name.clone(),
            compiler_version: self.version.clone(),
            compiler_settings: self.compiler_settings.to_string(),
            source_type: source_type.into(),
            source_files: self.sources.clone(),
            abi,
            constructor_arguments,
            match_type: match_type.into(),
            compilation_artifacts: Some(self.compilation_artifacts.to_string()),
            creation_input_artifacts: Some(self.creation_code_artifacts.to_string()),
            deployed_bytecode_artifacts: Some(self.runtime_code_artifacts.to_string()),
        }
    }

    fn to_verifier_extra_data(&self) -> smart_contract_verifier_v2::verify_response::ExtraData {
        let parse_code_parts = |code: Bytes, code_artifacts: serde_json::Value| {
            #[derive(Clone, Debug, Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct CborAuxdata {
                pub offset: usize,
                pub value: DisplayBytes,
            }

            #[derive(Clone, Debug, Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct CodeArtifacts {
                pub cbor_auxdata: BTreeMap<String, CborAuxdata>,
            }

            let code_artifacts: CodeArtifacts =
                serde_json::from_value(code_artifacts).expect("parsing code artifacts failed");
            let ordered_auxdata = {
                let mut auxdata = code_artifacts
                    .cbor_auxdata
                    .into_values()
                    .collect::<Vec<_>>();
                auxdata.sort_by_key(|auxdata| auxdata.offset);
                auxdata
            };
            let ordered_auxdata_ranges = ordered_auxdata
                .into_iter()
                .map(|auxdata| (auxdata.offset..auxdata.offset + auxdata.value.len()))
                .collect::<Vec<_>>();

            let mut parts = Vec::new();
            let mut main_range_start = 0;
            for range in ordered_auxdata_ranges {
                if !(main_range_start..range.start).is_empty() {
                    let main_part =
                        smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                            r#type: "main".to_string(),
                            data: DisplayBytes::from(code[main_range_start..range.start].to_vec())
                                .to_string(),
                        };
                    parts.push(main_part);
                }
                main_range_start = range.end;

                let metadata_part =
                    smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                        r#type: "meta".to_string(),
                        data: DisplayBytes::from(code[range].to_vec()).to_string(),
                    };
                parts.push(metadata_part);
            }
            parts
        };

        smart_contract_verifier_v2::verify_response::ExtraData {
            local_creation_input_parts: parse_code_parts(
                self.compiled_creation_code.0.clone(),
                self.creation_code_artifacts.clone(),
            ),
            local_deployed_bytecode_parts: parse_code_parts(
                self.compiled_runtime_code.0.clone(),
                self.runtime_code_artifacts.clone(),
            ),
        }
    }
}
