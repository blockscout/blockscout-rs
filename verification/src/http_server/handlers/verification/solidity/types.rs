use ethers_solc::{
    artifacts::{Libraries, Settings},
    CompilerInput,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Deserialize, PartialEq)]
pub struct VerificationRequest<T> {
    pub contract_name: String,
    pub deployed_bytecode: String,
    pub creation_bytecode: String,
    pub compiler_version: String,
    pub constructor_arguments: Option<String>,

    #[serde(flatten)]
    pub content: T,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct VerificationResponse {
    pub verified: bool,
}

type EvmVersion = ethers_solc::EvmVersion;

#[derive(Debug, Deserialize, PartialEq)]
struct ContractLibrary {
    lib_name: String,
    lib_address: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct FlattenedSource {
    source_code: String,
    evm_version: EvmVersion,
    optimization_runs: Option<usize>,
    contract_libraries: Option<Vec<ContractLibrary>>,
}

impl std::convert::From<FlattenedSource> for CompilerInput {
    fn from(source: FlattenedSource) -> Self {
        let mut settings = Settings::default();
        settings.optimizer.enabled = source.optimization_runs.map(|_| true);
        settings.optimizer.runs = source.optimization_runs;
        if let Some(source_libraries) = source.contract_libraries {
            let libraries = BTreeMap::from_iter(
                source_libraries
                    .into_iter()
                    .map(|l| (l.lib_name, l.lib_address)),
            );
            settings.libraries = Libraries {
                libs: BTreeMap::from([(PathBuf::from("source.sol"), libraries)]),
            };
        }
        CompilerInput {
            language: "Solidity".to_string(),
            sources: BTreeMap::from([(
                PathBuf::from("source.sol"),
                ethers_solc::artifacts::Source {
                    content: source.source_code,
                },
            )]),
            settings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use std::fmt::Debug;

    fn test_parse_ok<T>(tests: Vec<(&str, T)>)
    where
        T: Debug + PartialEq + DeserializeOwned,
    {
        for (s, value) in tests {
            let v: T = serde_json::from_str(s).unwrap();
            assert_eq!(v, value);
        }
    }

    #[test]
    fn verification_request() {
        test_parse_ok(vec![(
            r#"{
                    "contract_name": "test",
                    "deployed_bytecode": "0x6001",
                    "creation_bytecode": "0x6001",
                    "compiler_version": "test",
                    "source_code": "pragma",
                    "evm_version": "london",
                    "optimization_runs": 200
                }"#,
            VerificationRequest::<FlattenedSource> {
                contract_name: "test".into(),
                deployed_bytecode: "0x6001".into(),
                creation_bytecode: "0x6001".into(),
                compiler_version: "test".into(),
                constructor_arguments: None,
                content: FlattenedSource {
                    source_code: "pragma".into(),
                    evm_version: EvmVersion::London,
                    optimization_runs: Some(200),
                    contract_libraries: None,
                },
            },
        )])
    }
}
