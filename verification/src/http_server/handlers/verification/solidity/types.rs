use ethers_solc::{
    artifacts::{Libraries, Settings},
    CompilerInput, EvmVersion,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf, str::FromStr};

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

#[derive(Debug, Deserialize, PartialEq)]
struct ContractLibrary {
    lib_name: String,
    lib_address: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct FlattenedSource {
    source_code: String,
    evm_version: String,
    optimization_runs: Option<usize>,
    contract_libraries: Option<Vec<ContractLibrary>>,
}

impl TryFrom<FlattenedSource> for CompilerInput {
    type Error = anyhow::Error;

    fn try_from(source: FlattenedSource) -> Result<Self, Self::Error> {
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
        if source.evm_version != "default" {
            settings.evm_version =
                Some(EvmVersion::from_str(&source.evm_version).map_err(anyhow::Error::msg)?);
        }
        Ok(CompilerInput {
            language: "Solidity".to_string(),
            sources: BTreeMap::from([(
                PathBuf::from("source.sol"),
                ethers_solc::artifacts::Source {
                    content: source.source_code,
                },
            )]),
            settings,
        })
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
    fn parse_flattened() {
        test_parse_ok(vec![(
            r#"{
                    "contract_name": "test",
                    "deployed_bytecode": "0x6001",
                    "creation_bytecode": "0x6001",
                    "compiler_version": "0.8.3",
                    "source_code": "pragma",
                    "evm_version": "london",
                    "optimization_runs": 200
                }"#,
            VerificationRequest::<FlattenedSource> {
                contract_name: "test".into(),
                deployed_bytecode: "0x6001".into(),
                creation_bytecode: "0x6001".into(),
                compiler_version: "0.8.3".into(),
                constructor_arguments: None,
                content: FlattenedSource {
                    source_code: "pragma".into(),
                    evm_version: format!("{}", ethers_solc::EvmVersion::London),
                    optimization_runs: Some(200),
                    contract_libraries: None,
                },
            },
        )])
    }

    fn test_to_input(flatten: FlattenedSource, expected: &str) {
        let input: CompilerInput = flatten.try_into().unwrap();
        let input_json = serde_json::to_string(&input).unwrap();
        println!("{}", input_json);
        assert_eq!(input_json, expected);
    }

    #[test]
    fn flattened_to_input() {
        let flatten = FlattenedSource {
            source_code: "pragma".into(),
            evm_version: format!("{}", ethers_solc::EvmVersion::London),
            optimization_runs: Some(200),
            contract_libraries: Some(vec![ContractLibrary {
                lib_name: "some_library".into(),
                lib_address: "some_address".into(),
            }]),
        };
        let expected = r#"{"language":"Solidity","sources":{"source.sol":{"content":"pragma"}},"settings":{"optimizer":{"enabled":true,"runs":200},"outputSelection":{"*":{"":["ast"],"*":["abi","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}},"evmVersion":"london","libraries":{"source.sol":{"some_library":"some_address"}}}}"#;
        test_to_input(flatten, expected);
        let flatten = FlattenedSource {
            source_code: "".into(),
            evm_version: format!("{}", ethers_solc::EvmVersion::SpuriousDragon),
            optimization_runs: None,
            contract_libraries: None,
        };
        let expected = r#"{"language":"Solidity","sources":{"source.sol":{"content":""}},"settings":{"optimizer":{},"outputSelection":{"*":{"":["ast"],"*":["abi","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}},"evmVersion":"london"}}"#;
        test_to_input(flatten, expected);
    }
}
