use blockscout_display_bytes::Bytes as DisplayBytes;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::BytecodeType;
use std::collections::BTreeMap;

const TEST_CASES_DIR: &str = "tests/test_cases_vyper";

pub trait TestCase {
    fn to_request(&self) -> serde_json::Value;

    fn contract_name(&self) -> &str;

    fn constructor_args(&self) -> Option<DisplayBytes>;

    fn compiler_version(&self) -> &str;

    fn source_files(&self) -> BTreeMap<String, String>;

    fn evm_version(&self) -> Option<String> {
        None
    }

    fn optimize(&self) -> Option<bool> {
        None
    }

    fn bytecode_metadata(&self) -> Option<bool> {
        None
    }
}

pub fn from_file<T: TestCase + DeserializeOwned>(test_case: &str) -> T {
    let test_case_path = format!("{TEST_CASES_DIR}/{test_case}.json");
    let content = std::fs::read_to_string(test_case_path).expect("failed to read file");
    serde_json::from_str(&content).expect("invalid test case format")
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Flattened {
    #[serde(default = "default_flattened_contract_name")]
    pub contract_name: String,
    pub deployed_bytecode: String,
    pub creation_bytecode: String,
    pub compiler_version: String,
    pub source_code: String,
    pub expected_constructor_argument: Option<DisplayBytes>,
}

fn default_flattened_contract_name() -> String {
    "VyperContract".to_string()
}

impl Flattened {}

impl TestCase for Flattened {
    fn to_request(&self) -> serde_json::Value {
        serde_json::json!({
            "bytecode": self.creation_bytecode,
            "bytecodeType": BytecodeType::CreationInput.as_str_name(),
            "compilerVersion": self.compiler_version,
            "sourceFiles": {
                format!("{}.vy", self.contract_name): self.source_code
            },
        })
    }

    fn contract_name(&self) -> &str {
        self.contract_name.as_str()
    }

    fn constructor_args(&self) -> Option<DisplayBytes> {
        self.expected_constructor_argument.clone()
    }

    fn compiler_version(&self) -> &str {
        self.compiler_version.as_str()
    }

    fn source_files(&self) -> BTreeMap<String, String> {
        let file_name = format!("{}.vy", self.contract_name);
        BTreeMap::from([(file_name, self.source_code.clone())])
    }
}
