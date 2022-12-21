use crate::proto::{source, Source};
use smart_contract_verifier::{MatchType, SourcifySuccess, VerificationSuccess};

pub fn from_verification_success(value: VerificationSuccess) -> Source {
    let compiler_input = value.compiler_input;
    let compiler_settings = serde_json::to_string(&compiler_input.settings)
        .expect("Is result of local compilation and, thus, should be always valid");

    let source_type = if value.file_path.ends_with(".sol") {
        source::SourceType::Solidity
    } else if value.file_path.ends_with(".yul") {
        source::SourceType::Yul
    } else if value.file_path.ends_with(".vy") {
        source::SourceType::Vyper
    } else {
        source::SourceType::Unspecified
    };

    let match_type = match value.match_type {
        MatchType::Partial => source::Match::Partial,
        MatchType::Full => source::Match::Full,
    };

    Source {
        file_name: value.file_path,
        contract_name: value.contract_name,
        compiler_version: value.compiler_version.to_string(),
        compiler_settings,
        source_type: source_type.into(),
        source_files: compiler_input
            .sources
            .into_iter()
            .map(|(path, source)| {
                (source::SourceFile {
                    name: path.to_string_lossy().to_string(),
                    content: source.content,
                })
            })
            .collect(),
        abi: value.abi.as_ref().map(|abi| {
            serde_json::to_string(abi)
                .expect("Is result of local compilation and, thus, should be always valid")
        }),
        constructor_arguments: value.constructor_args.map(|args| args.to_string()),
        r#match: match_type.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockscout_display_bytes::Bytes as DisplayBytes;
    use std::collections::BTreeMap;
    use std::str::FromStr;
    use ethers_solc::artifacts::{self, Libraries, Optimizer, Settings};
    use ethers_solc::{CompilerInput, EvmVersion};
    use smart_contract_verifier::Version;

    #[test]
    fn test_from_verification_success() {
        let compiler_settings = Settings {
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
        let verification_success = VerificationSuccess {
            compiler_input: CompilerInput {
                language: "Solidity".to_string(),
                sources: BTreeMap::from([(
                    "file.sol".into(),
                    artifacts::Source {
                        content: "content".into(),
                    },
                )]),
                settings: compiler_settings.clone(),
            },
            compiler_output: Default::default(),
            compiler_version: Version::from_str("v0.8.17+commit.8df45f5f").unwrap(),
            file_path: "file.sol".to_string(),
            contract_name: "contract_name".to_string(),
            abi: Some(Default::default()),
            constructor_args: Some(DisplayBytes::from_str("0x123456").unwrap()),
            local_bytecode_parts: Default::default(),
            match_type: MatchType::Partial,
        };

        let result = from_verification_success(verification_success);

        let expected = Source {
            file_name: "file.sol".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            compiler_settings: serde_json::to_string(&compiler_settings).unwrap(),
            source_type: source::SourceType::Solidity.into(),
            source_files: Vec::from([source::SourceFile {name: "file.sol".into(), content: "content".into()}]),
            constructor_arguments: Some("0x123456".into()),
            abi: Some(serde_json::to_string(&ethabi::Contract::default()).unwrap()),
            r#match: source::Match::Partial.into(),
        };

        assert_eq!(expected, result);
    }

//     #[test]
//     fn from_sourcify_success() {
//         let verification_success = SourcifySuccess {
//             file_name: "file_name".to_string(),
//             compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
//             evm_version: "london".to_string(),
//             optimization: Some(true),
//             optimization_runs: Some(200),
//             constructor_arguments: Some(DisplayBytes::from_str("0x123456").unwrap().0),
//             contract_name: "contract_name".to_string(),
//             abi: "abi".to_string(),
//             sources: BTreeMap::from([("path".into(), "content".into())]),
//             contract_libraries: BTreeMap::from([("lib_name".into(), "lib_address".into())]),
//             compiler_settings: "compiler_settings".to_string(),
//             match_type: MatchType::Full,
//         };
//         let result = ResultWrapper::from(verification_success).into_inner();
//
//         let expected = Result {
//             file_name: "file_name".to_string(),
//             contract_name: "contract_name".to_string(),
//             compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
//             sources: BTreeMap::from([("path".into(), "content".into())]),
//             evm_version: "london".to_string(),
//             optimization: Some(true),
//             optimization_runs: Some(200),
//             contract_libraries: BTreeMap::from([("lib_name".into(), "lib_address".into())]),
//             compiler_settings: "compiler_settings".to_string(),
//             constructor_arguments: Some("0x123456".into()),
//             abi: Some("abi".to_string()),
//             local_creation_input_parts: vec![],
//             local_deployed_bytecode_parts: vec![],
//             match_type: 2,
//         };
//
//         assert_eq!(expected, result);
//     }
}

// local_creation_input_parts: value
//     .local_bytecode_parts
//     .creation_tx_input_parts
//     .into_iter()
//     .map(|part| result::BytecodePartWrapper::from(part).into_inner())
//     .collect(),
// local_deployed_bytecode_parts: value
//     .local_bytecode_parts
//     .deployed_bytecode_parts
//     .into_iter()
//     .map(|part| result::BytecodePartWrapper::from(part).into_inner())
//     .collect(),

