use std::collections::BTreeMap;

use ethers_solc::EvmVersion;
use serde::Deserialize;

use crate::VerificationResult;

use super::types::Files;

const METADATA_FILE_NAME: &str = "metadata.json";

#[derive(Debug, PartialEq, Deserialize)]

pub struct Metadata {
    pub settings: MetadataSettings,
    pub compiler: Compiler,
    pub output: Output,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSettings {
    pub compilation_target: BTreeMap<String, String>,
    pub optimizer: Optimizer,
    pub libraries: BTreeMap<String, String>,
    #[serde(rename = "camelCase")]
    pub evm_version: Option<EvmVersion>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Optimizer {
    pub enabled: Option<bool>,
    pub runs: Option<usize>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Compiler {
    pub version: String,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Output {
    pub abi: serde_json::Value,
}

impl TryFrom<&Files> for Metadata {
    type Error = anyhow::Error;

    fn try_from(files: &Files) -> Result<Self, Self::Error> {
        let metadata_content = files
            .0
            .get(METADATA_FILE_NAME)
            .ok_or_else(|| anyhow::Error::msg(format!("file {} not found", METADATA_FILE_NAME)))?;

        serde_json::from_str(metadata_content.as_str()).map_err(anyhow::Error::msg)
    }
}

impl TryFrom<Files> for VerificationResult {
    type Error = anyhow::Error;

    fn try_from(files: Files) -> Result<Self, Self::Error> {
        let metadata = Metadata::try_from(&files)?;
        let contract_name = metadata
            .settings
            .compilation_target
            .iter()
            .next()
            .ok_or_else(|| anyhow::Error::msg("compilation target not found"))?
            .1
            .to_string();
        let compiler_version = metadata.compiler.version;
        let evm_version = metadata
            .settings
            .evm_version
            .unwrap_or_default()
            .to_string();
        let optimization_runs = metadata.settings.optimizer.enabled.and_then(|enabled| {
            if enabled {
                metadata.settings.optimizer.runs
            } else {
                None
            }
        });
        let contract_libraries = metadata.settings.libraries;
        let abi = serde_json::to_string(&metadata.output.abi)?;
        let sources = files
            .0
            .into_iter()
            .filter(|(name, _)| !name.ends_with(METADATA_FILE_NAME))
            .collect();

        Ok(VerificationResult {
            contract_name,
            compiler_version,
            evm_version,
            // TODO: extract args
            constructor_arguments: None,
            contract_libraries,
            optimization_runs,
            abi,
            sources,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_METADATA: &str = r#"{
        "compiler": {
            "version": "0.8.14+commit.80d49f37"
        },
        "output": {
            "abi": [{"inputs":[],"name":"retrieve","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"}]
        },
        "settings": {
            "compilationTarget": {
                "example.sol": "Example"
            },
            "evmVersion": "london",
            "libraries": {
                "SafeMath": "0xFBe36e5cAD207d5fDee40E6568bb276a351f6713"
            },
            "optimizer": {
                "enabled": false,
                "runs": 200
            }
        }
    }"#;

    #[test]
    fn parse_metadata_from_files() {
        let mut files = Files(BTreeMap::from([
            ("source.sol".into(), "content".into()),
            (METADATA_FILE_NAME.into(), DEFAULT_METADATA.into()),
        ]));

        let meta = Metadata::try_from(&files);
        assert!(
            meta.is_ok(),
            "Parse metadata from files failed: {}",
            meta.unwrap_err()
        );

        files.0.remove(METADATA_FILE_NAME.into());
        let meta = Metadata::try_from(&files);
        assert!(meta.is_err(), "Parsing files without metadata should fail",);
    }

    #[test]
    fn parse_response_from_files() {
        let files = Files(BTreeMap::from([
            ("source.sol".into(), "content".into()),
            (METADATA_FILE_NAME.into(), DEFAULT_METADATA.into()),
        ]));

        let verification_result = VerificationResult::try_from(files);
        assert!(
            verification_result.is_ok(),
            "Parse response from files failed: {}",
            verification_result.unwrap_err()
        );
        assert_eq!(
            verification_result.unwrap(),
            VerificationResult {
                contract_name: "Example".into(),
                compiler_version: "0.8.14+commit.80d49f37".into(),
                evm_version: "london".into(),
                constructor_arguments: None,
                contract_libraries: BTreeMap::from([("SafeMath".into(), "0xFBe36e5cAD207d5fDee40E6568bb276a351f6713".into())]),
                optimization_runs: None,
                abi: r#"[{"inputs":[],"name":"retrieve","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"}]"#.into(),
                sources: BTreeMap::from([("source.sol".into(), "content".into())]),
            }
        );

        let files = Files(BTreeMap::from([("source.sol".into(), "content".into())]));

        let verification_result = VerificationResult::try_from(files);
        assert!(
            verification_result.is_err(),
            "Parsing files without metadata should fail",
        );
    }
}
