use super::types::{Files, Success};
use ethers_solc::EvmVersion;
use serde::Deserialize;
use std::collections::BTreeMap;

const METADATA_FILE_NAME: &str = "metadata.json";

// There is struct for metadata in ethers_solc::artifacts::Metadata
// however it is for standard json input of compiler and
// has different `libraries` field structure
#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Metadata {
    pub settings: MetadataSettings,
    pub compiler: Compiler,
    pub output: Output,

    // Is not deserialized and should be filled manually
    #[serde(skip)]
    pub raw_settings: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSettings {
    pub compilation_target: BTreeMap<String, String>,
    pub optimizer: Optimizer,
    pub libraries: BTreeMap<String, String>,
    #[serde(rename = "camelCase")]
    pub evm_version: Option<EvmVersion>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Optimizer {
    pub enabled: Option<bool>,
    pub runs: Option<usize>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Compiler {
    pub version: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Output {
    pub abi: serde_json::Value,
}

impl Files {
    fn extract_metadata_and_source_files(
        self,
    ) -> Result<(Metadata, BTreeMap<String, String>), anyhow::Error> {
        let metadata_content = self
            .0
            .get(METADATA_FILE_NAME)
            .ok_or_else(|| anyhow::Error::msg(format!("file {} not found", METADATA_FILE_NAME)))?;
        let metadata = {
            let mut metadata: Metadata =
                serde_json::from_str(metadata_content).map_err(anyhow::Error::msg)?;
            let raw_metadata: serde_json::Value = serde_json::from_str(metadata_content)?;
            metadata.raw_settings = format!("{}", raw_metadata["settings"]);
            metadata
        };

        let source_files: BTreeMap<String, String> = self
            .0
            .into_iter()
            .filter(|(name, _)| !name.ends_with(METADATA_FILE_NAME))
            .collect();

        Ok((metadata, source_files))
    }
}

impl TryFrom<Files> for Success {
    type Error = anyhow::Error;

    fn try_from(files: Files) -> Result<Self, Self::Error> {
        let (metadata, source_files) = files.extract_metadata_and_source_files()?;

        let compiler_version = metadata.compiler.version;
        let (file_name, contract_name) = metadata
            .settings
            .compilation_target
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::Error::msg("compilation target not found"))?;
        let evm_version = metadata
            .settings
            .evm_version
            .unwrap_or_default()
            .to_string();
        let optimization = metadata.settings.optimizer.enabled;
        let optimization_runs = metadata.settings.optimizer.runs;
        let contract_libraries: BTreeMap<String, String> = metadata.settings.libraries;
        let abi = serde_json::to_string(&metadata.output.abi)?;

        Ok(Success {
            file_name,
            contract_name,
            compiler_version,
            evm_version,
            // TODO: extract args
            constructor_arguments: None,
            contract_libraries,
            optimization,
            optimization_runs,
            abi,
            sources: source_files,
            compiler_settings: metadata.raw_settings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
        let files = Files(BTreeMap::from([
            ("source.sol".into(), "content".into()),
            (METADATA_FILE_NAME.into(), DEFAULT_METADATA.into()),
        ]));
        let result = files.extract_metadata_and_source_files();

        let (_, files) = result.expect("parse metadata from files failed");
        assert_eq!(
            files,
            BTreeMap::from([("source.sol".into(), "content".into())]),
        );

        let files = Files(BTreeMap::from([("source.sol".into(), "content".into())]));
        files
            .extract_metadata_and_source_files()
            .expect_err("Parsing files without metadata should fail");
    }

    #[test]
    fn parse_response_from_files() {
        let files = Files(BTreeMap::from([
            ("source.sol".into(), "content".into()),
            (METADATA_FILE_NAME.into(), DEFAULT_METADATA.into()),
        ]));

        let verification_result =
            Success::try_from(files).expect("parse response from files failed");
        assert_eq!(
            verification_result,
            Success {
                file_name: "example.sol".into(),
                contract_name: "Example".into(),
                compiler_version: "0.8.14+commit.80d49f37".into(),
                evm_version: "london".into(),
                constructor_arguments: None,
                contract_libraries: BTreeMap::from([("SafeMath".into(), "0xFBe36e5cAD207d5fDee40E6568bb276a351f6713".into())]),
                optimization: Some(false),
                optimization_runs: Some(200),
                abi: r#"[{"inputs":[],"name":"retrieve","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"}]"#.into(),
                sources: BTreeMap::from([("source.sol".into(), "content".into())]),
                compiler_settings: "{\"compilationTarget\":{\"example.sol\":\"Example\"},\"evmVersion\":\"london\",\"libraries\":{\"SafeMath\":\"0xFBe36e5cAD207d5fDee40E6568bb276a351f6713\"},\"optimizer\":{\"enabled\":false,\"runs\":200}}".to_string()
            }
        );

        let files = Files(BTreeMap::from([("source.sol".into(), "content".into())]));
        Success::try_from(files).expect_err("Parsing files without metadata should fail");
    }
}
