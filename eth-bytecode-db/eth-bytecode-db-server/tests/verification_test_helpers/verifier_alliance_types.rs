use super::test_input_data::TestInputData;
use blockscout_display_bytes::Bytes as DisplayBytes;
use bytes::Bytes;
use eth_bytecode_db::verification::{BytecodeType, VerificationMetadata, VerificationRequest};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use serde::Deserialize;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Debug, Clone, Deserialize)]
pub struct TestCase {
    #[serde(skip)]
    pub test_case_name: String,

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
    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        // e.g. "tests/alliance_test_cases/full_match.json" => "full_match"
        let test_case_name = PathBuf::from(path.as_ref())
            .file_stem()
            .as_ref()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let content = std::fs::read_to_string(path).expect("failed to read file");
        let mut test_case: TestCase =
            serde_json::from_str(&content).expect("invalid test case format");
        test_case.test_case_name = test_case_name;
        test_case
    }

    pub fn to_test_input_data(&self) -> TestInputData {
        let eth_bytecode_db_source = self.to_eth_bytecode_db_source();
        let verifier_extra_data = self.to_verifier_extra_data();

        TestInputData::from_source_and_extra_data(eth_bytecode_db_source, verifier_extra_data)
    }

    fn to_eth_bytecode_db_source(&self) -> eth_bytecode_db_v2::Source {
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
            .as_ref()
            .and_then(|values| {
                values
                    .as_object()
                    .expect("`creation_values` must be an object")
                    .get("constructorArguments")
            })
            .map(|args| {
                args.as_str()
                    .expect("`constructorArguments` must be a string")
                    .to_string()
            });
        let match_type = if self
            .creation_values
            .as_ref()
            .and_then(|values| values.as_object().unwrap().get("cborAuxdata"))
            .is_some()
        {
            smart_contract_verifier_v2::source::MatchType::Partial
        } else {
            smart_contract_verifier_v2::source::MatchType::Full
        };

        eth_bytecode_db_v2::Source {
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
