use ethers_solc::{
    artifacts::{Libraries, Settings, Source, Sources},
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

#[derive(Debug, Deserialize, PartialEq)]
pub struct MultiPartFiles {
    sources: BTreeMap<PathBuf, String>,
    evm_version: String,
    optimization_runs: Option<usize>,
    contract_libraries: Option<BTreeMap<String, String>>,
}

impl TryFrom<MultiPartFiles> for CompilerInput {
    type Error = anyhow::Error;

    fn try_from(multi_part: MultiPartFiles) -> Result<Self, Self::Error> {
        let mut settings = Settings::default();
        settings.optimizer.enabled = Some(multi_part.optimization_runs.is_some());
        settings.optimizer.runs = multi_part.optimization_runs;
        if let Some(libs) = multi_part.contract_libraries {
            // we have to know filename for library, but we don't know,
            // so we assume that every file MAY contains all libraries
            let libs = multi_part
                .sources
                .iter()
                .map(|(filename, _)| (PathBuf::from(filename), libs.clone()))
                .collect();
            settings.libraries = Libraries { libs };
        }

        if multi_part.evm_version != "default" {
            settings.evm_version =
                Some(EvmVersion::from_str(&multi_part.evm_version).map_err(anyhow::Error::msg)?);
        } else {
            // `Settings::default()` sets the value to the latest available evm version (`Some(London)` for now)
            settings.evm_version = None
        }

        let sources: Sources = multi_part
            .sources
            .into_iter()
            .map(|(name, content)| (name, Source { content }))
            .collect();
        Ok(CompilerInput {
            language: "Solidity".to_string(),
            sources,
            settings,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct VersionsResponse {
    pub versions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::parse::test_deserialize_ok;

    fn sources(sources: &[(&str, &str)]) -> BTreeMap<PathBuf, String> {
        sources
            .iter()
            .map(|(name, content)| (PathBuf::from(name), content.to_string()))
            .collect()
    }

    #[test]
    fn parse_multi_part() {
        test_deserialize_ok(vec![
            (
                r#"{
                        "contract_name": "test",
                        "deployed_bytecode": "0x6001",
                        "creation_bytecode": "0x6001",
                        "compiler_version": "0.8.3",
                        "sources": {
                            "source.sol": "pragma"
                        },
                        "evm_version": "london",
                        "optimization_runs": 200
                    }"#,
                VerificationRequest::<MultiPartFiles> {
                    contract_name: "test".into(),
                    deployed_bytecode: "0x6001".into(),
                    creation_bytecode: "0x6001".into(),
                    compiler_version: "0.8.3".into(),
                    constructor_arguments: None,
                    content: MultiPartFiles {
                        sources: sources(&[("source.sol", "pragma")]),
                        evm_version: format!("{}", ethers_solc::EvmVersion::London),
                        optimization_runs: Some(200),
                        contract_libraries: None,
                    },
                },
            ),
            (
                r#"{
                    "contract_name": "test",
                    "deployed_bytecode": "0x6001",
                    "creation_bytecode": "0x6001",
                    "compiler_version": "0.8.3",
                    "sources": {
                        "source.sol": "source",
                        "A.sol": "A",
                        "B": "B",
                        "metadata.json": "metadata"
                    },
                    "evm_version": "spuriousDragon",
                    "contract_libraries": {
                        "Lib.sol": "0x1234567890123456789012345678901234567890"
                    }
                }"#,
                VerificationRequest::<MultiPartFiles> {
                    contract_name: "test".into(),
                    deployed_bytecode: "0x6001".into(),
                    creation_bytecode: "0x6001".into(),
                    compiler_version: "0.8.3".into(),
                    constructor_arguments: None,
                    content: MultiPartFiles {
                        sources: sources(&[
                            ("source.sol", "source"),
                            ("A.sol", "A"),
                            ("B", "B"),
                            ("metadata.json", "metadata"),
                        ]),
                        evm_version: format!("{}", ethers_solc::EvmVersion::SpuriousDragon),
                        optimization_runs: None,
                        contract_libraries: Some(BTreeMap::from([(
                            "Lib.sol".into(),
                            "0x1234567890123456789012345678901234567890".into(),
                        )])),
                    },
                },
            ),
        ])
    }

    fn test_to_input(multi_part: MultiPartFiles, expected: &str) {
        let input: CompilerInput = multi_part.try_into().unwrap();
        let input_json = serde_json::to_string(&input).unwrap();
        println!("{}", input_json);
        assert_eq!(input_json, expected);
    }

    #[test]
    fn multi_part_to_input() {
        let mutli_part = MultiPartFiles {
            sources: sources(&[("source.sol", "pragma")]),
            evm_version: format!("{}", ethers_solc::EvmVersion::London),
            optimization_runs: Some(200),
            contract_libraries: Some(BTreeMap::from([(
                "some_library".into(),
                "some_address".into(),
            )])),
        };
        let expected = r#"{"language":"Solidity","sources":{"source.sol":{"content":"pragma"}},"settings":{"optimizer":{"enabled":true,"runs":200},"outputSelection":{"*":{"":["ast"],"*":["abi","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}},"evmVersion":"london","libraries":{"source.sol":{"some_library":"some_address"}}}}"#;
        test_to_input(mutli_part, expected);
        let multi_part = MultiPartFiles {
            sources: sources(&[("source.sol", "")]),
            evm_version: format!("{}", ethers_solc::EvmVersion::SpuriousDragon),
            optimization_runs: None,
            contract_libraries: None,
        };
        let expected = r#"{"language":"Solidity","sources":{"source.sol":{"content":""}},"settings":{"optimizer":{"enabled":false},"outputSelection":{"*":{"":["ast"],"*":["abi","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}},"evmVersion":"spuriousDragon","libraries":{}}}"#;
        test_to_input(multi_part, expected);
    }

    #[test]
    // 'default' should result in None in CompilerInput
    fn default_evm_version() {
        let multi_part = MultiPartFiles {
            sources: BTreeMap::new(),
            evm_version: "default".to_string(),
            optimization_runs: None,
            contract_libraries: None,
        };
        let compiler_input = CompilerInput::try_from(multi_part).expect("Structure is valid");
        assert_eq!(
            None, compiler_input.settings.evm_version,
            "'default' should result in `None`"
        )
    }
}
