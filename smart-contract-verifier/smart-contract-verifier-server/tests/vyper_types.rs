use blockscout_display_bytes::Bytes as DisplayBytes;
use serde::{de::DeserializeOwned, Deserialize};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::BytecodeType;
use std::{borrow::Cow, collections::BTreeMap, path::PathBuf, str::FromStr};

const TEST_CASES_DIR: &str = "tests/test_cases_vyper";

pub trait TestCase {
    fn to_request(&self) -> serde_json::Value;

    fn file_name(&self) -> Cow<'_, str>;

    fn contract_name(&self) -> &str;

    fn constructor_args(&self) -> Option<DisplayBytes>;

    fn compiler_version(&self) -> &str;

    fn source_files(&self) -> BTreeMap<String, String>;

    fn evm_version(&self) -> Option<String>;

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

#[derive(Debug, Clone, Deserialize)]
pub struct Flattened {
    #[serde(default = "default_flattened_contract_name")]
    pub contract_name: String,
    pub deployed_bytecode: String,
    pub creation_bytecode: String,
    pub compiler_version: String,
    pub evm_version: Option<String>,
    pub source_code: String,
    pub expected_constructor_argument: Option<DisplayBytes>,
}

fn default_flattened_contract_name() -> String {
    "VyperContract".to_string()
}

impl TestCase for Flattened {
    fn to_request(&self) -> serde_json::Value {
        serde_json::json!({
            "bytecode": self.creation_bytecode,
            "bytecodeType": BytecodeType::CreationInput.as_str_name(),
            "compilerVersion": self.compiler_version,
            "evmVersion": self.evm_version,
            "sourceFiles": {
                format!("{}.vy", self.contract_name): self.source_code
            },
        })
    }

    fn file_name(&self) -> Cow<'_, str> {
        format!("{}.vy", self.contract_name).into()
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

    fn evm_version(&self) -> Option<String> {
        self.evm_version.clone()
    }

    fn source_files(&self) -> BTreeMap<String, String> {
        BTreeMap::from([(self.file_name().to_string(), self.source_code.clone())])
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MultiPart {
    pub deployed_bytecode: String,
    pub creation_bytecode: String,
    pub compiler_version: String,
    pub file_name: String,
    pub contract_name: String,
    pub evm_version: Option<String>,
    pub source_files: BTreeMap<String, String>,
    pub interfaces: BTreeMap<String, String>,
    pub expected_constructor_argument: Option<DisplayBytes>,
}

impl TestCase for MultiPart {
    fn to_request(&self) -> serde_json::Value {
        serde_json::json!({
            "bytecode": self.creation_bytecode,
            "bytecodeType": BytecodeType::CreationInput.as_str_name(),
            "compilerVersion": self.compiler_version,
            "evmVersion": self.evm_version,
            "sourceFiles": self.source_files,
            "interfaces": self.interfaces,
        })
    }

    fn file_name(&self) -> Cow<'_, str> {
        Cow::from(&self.file_name)
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

    fn evm_version(&self) -> Option<String> {
        self.evm_version.clone()
    }

    fn source_files(&self) -> BTreeMap<String, String> {
        let sources = self.source_files.clone().into_iter();
        let interfaces = self.interfaces.iter().map(|(path, content)| {
            let content = if PathBuf::from(path).extension() == Some(std::ffi::OsStr::new("json")) {
                let value = serde_json::Value::from_str(content).unwrap();
                value.to_string()
            } else {
                content.clone()
            };
            (path.clone(), content)
        });
        sources.chain(interfaces).collect()
    }
}
