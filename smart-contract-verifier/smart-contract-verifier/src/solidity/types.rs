use crate::{
    compiler,
    verifier::{self, LocalBytecodeParts},
    MatchType,
};
use blockscout_display_bytes::Bytes as DisplayBytes;
use ethers_solc::CompilerOutput;

#[derive(Clone, Debug)]
pub struct Success {
    pub compiler_input: ethers_solc::CompilerInput,
    pub compiler_output: CompilerOutput,
    pub compiler_version: compiler::Version,
    pub file_path: String,
    pub contract_name: String,
    pub abi: Option<serde_json::Value>,
    pub constructor_args: Option<DisplayBytes>,
    pub local_bytecode_parts: LocalBytecodeParts,
    pub match_type: MatchType,
    pub compilation_artifacts: serde_json::Value,
    pub creation_input_artifacts: serde_json::Value,
    pub deployed_bytecode_artifacts: serde_json::Value,
}

impl From<(ethers_solc::CompilerInput, verifier::Success)> for Success {
    fn from((compiler_input, success): (ethers_solc::CompilerInput, verifier::Success)) -> Self {
        Self {
            compiler_input,
            compiler_output: success.compiler_output,
            compiler_version: success.compiler_version,
            file_path: success.file_path,
            contract_name: success.contract_name,
            abi: success.abi,
            constructor_args: success.constructor_args,
            local_bytecode_parts: success.local_bytecode_parts,
            match_type: success.match_type,
            compilation_artifacts: success.compilation_artifacts,
            creation_input_artifacts: success.creation_input_artifacts,
            deployed_bytecode_artifacts: success.deployed_bytecode_artifacts,
        }
    }
}

pub mod proto {
    use super::Success;
    use crate::common_types::from_success;
    use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{source, Source};
    use std::sync::Arc;

    impl From<Success> for Source {
        fn from(value: Success) -> Self {
            let source_type = match value.compiler_input.language.as_str() {
                "Solidity" => source::SourceType::Solidity,
                "Yul" => source::SourceType::Yul,
                _ => source::SourceType::Unspecified,
            };
            let extract_source_files = |compiler_input: ethers_solc::CompilerInput| {
                compiler_input
                    .sources
                    .into_iter()
                    .map(|(path, source)| {
                        // Similar to `unwrap_or_clone` which is still nightly-only feature.
                        let content = Arc::try_unwrap(source.content)
                            .unwrap_or_else(|content| (*content).clone());
                        (path.to_string_lossy().to_string(), content)
                    })
                    .collect()
            };
            from_success!(value, source_type, extract_source_files)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::Success;
        use crate::{MatchType, Version};
        use blockscout_display_bytes::Bytes as DisplayBytes;
        use ethers_solc::{
            artifacts,
            artifacts::{Libraries, Optimizer},
            EvmVersion,
        };
        use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
            source, Source,
        };
        use std::{collections::BTreeMap, str::FromStr};

        #[test]
        fn test_from_solidity_success() {
            let compiler_settings = artifacts::Settings {
                optimizer: Optimizer {
                    enabled: Some(true),
                    runs: Some(200),
                    ..Default::default()
                },
                evm_version: Some(EvmVersion::London),
                libraries: Libraries {
                    libs: BTreeMap::from([(
                        "lib_path".into(),
                        BTreeMap::from([("lib_name".into(), "lib_address".into())]),
                    )]),
                },
                ..Default::default()
            };
            let verification_success = Success {
                compiler_input: ethers_solc::CompilerInput {
                    language: "Solidity".to_string(),
                    sources: BTreeMap::from([(
                        "file_name".into(),
                        artifacts::Source::new("content"),
                    )]),
                    settings: compiler_settings.clone(),
                },
                compiler_output: Default::default(),
                compiler_version: Version::from_str("v0.8.17+commit.8df45f5f").unwrap(),
                file_path: "file_name".to_string(),
                contract_name: "contract_name".to_string(),
                abi: Some(serde_json::Value::Object(Default::default())),
                constructor_args: Some(DisplayBytes::from_str("0x123456").unwrap()),
                local_bytecode_parts: Default::default(),
                match_type: MatchType::Partial,
                compilation_artifacts: serde_json::json!({"abi": []}),
                creation_input_artifacts: serde_json::json!({"sourceMap": "-1:-1:0:-;;;;;:::-;;:::-;:::-;;;;;;;;;:::-;"}),
                deployed_bytecode_artifacts: serde_json::json!({"sourceMap": "1704:475;;;;:::-;-1:-1;;;;;;:::-;;"}),
            };

            let result = verification_success.into();

            let expected = Source {
                file_name: "file_name".to_string(),
                contract_name: "contract_name".to_string(),
                compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
                compiler_settings: serde_json::to_string(&compiler_settings).unwrap(),
                source_type: source::SourceType::Solidity.into(),
                source_files: BTreeMap::from([("file_name".into(), "content".into())]),
                constructor_arguments: Some("0x123456".into()),
                abi: Some("{}".to_string()),
                match_type: source::MatchType::Partial.into(),
                compilation_artifacts: Some("{\"abi\":[]}".into()),
                creation_input_artifacts: Some(
                    "{\"sourceMap\":\"-1:-1:0:-;;;;;:::-;;:::-;:::-;;;;;;;;;:::-;\"}".into(),
                ),
                deployed_bytecode_artifacts: Some(
                    "{\"sourceMap\":\"1704:475;;;;:::-;-1:-1;;;;;;:::-;;\"}".into(),
                ),
            };

            assert_eq!(expected, result);
        }
    }
}
