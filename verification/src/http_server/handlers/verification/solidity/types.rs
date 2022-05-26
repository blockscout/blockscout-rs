use ethers_solc::{
    artifacts::{Libraries, Settings},
    CompilerInput,
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

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct EvmVersion(
    #[serde(serialize_with = "use_display", deserialize_with = "use_from_str")]
    ethers_solc::EvmVersion,
);

fn use_from_str<'de, D: serde::Deserializer<'de>, T: FromStr<Err = String>>(
    deserializer: D,
) -> Result<T, D::Error> {
    let s: &str = serde::de::Deserialize::deserialize(deserializer)?;
    T::from_str(s).map_err(serde::de::Error::custom)
}

fn use_display<T: std::fmt::Display, S: serde::Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

impl From<ethers_solc::EvmVersion> for EvmVersion {
    fn from(v: ethers_solc::EvmVersion) -> Self {
        Self(v)
    }
}

impl From<EvmVersion> for ethers_solc::EvmVersion {
    fn from(v: EvmVersion) -> Self {
        v.0
    }
}

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

impl From<FlattenedSource> for CompilerInput {
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
                    evm_version: ethers_solc::EvmVersion::London.into(),
                    optimization_runs: Some(200),
                    contract_libraries: None,
                },
            },
        )])
    }

    fn test_to_input(flatten: FlattenedSource, expected: &str) {
        let input: CompilerInput = flatten.into();
        let input_json = serde_json::to_string(&input).unwrap();
        println!("{}", input_json);
        assert_eq!(input_json, expected);
    }

    #[test]
    fn flattened_to_input() {
        let flatten = FlattenedSource {
            source_code: "pragma".into(),
            evm_version: ethers_solc::EvmVersion::London.into(),
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
            evm_version: ethers_solc::EvmVersion::SpuriousDragon.into(),
            optimization_runs: None,
            contract_libraries: None,
        };
        let expected = r#"{"language":"Solidity","sources":{"source.sol":{"content":""}},"settings":{"optimizer":{},"outputSelection":{"*":{"":["ast"],"*":["abi","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}},"evmVersion":"london"}}"#;
        test_to_input(flatten, expected);
    }
}
