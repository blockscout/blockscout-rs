use crate::proto::{source, Source};
use blockscout_display_bytes::Bytes as DisplayBytes;
use smart_contract_verifier::{MatchType, SoliditySuccess, SourcifySuccess, VyperSuccess};
use std::sync::Arc;

macro_rules! from_success {
    ( $value:expr, $source_type:expr ) => {{
        let compiler_input = $value.compiler_input;
        let compiler_settings = serde_json::to_string(&compiler_input.settings)
            .expect("Is result of local compilation and, thus, should be always valid");

        let match_type = match $value.match_type {
            MatchType::Partial => source::MatchType::Partial,
            MatchType::Full => source::MatchType::Full,
        };

        Source {
            file_name: $value.file_path,
            contract_name: $value.contract_name,
            compiler_version: $value.compiler_version.to_string(),
            compiler_settings,
            source_type: $source_type.into(),
            source_files: compiler_input
                .sources
                .into_iter()
                .map(|(path, source)| {
                    // Similar to `unwrap_or_clone` which is still nightly-only feature.
                    let content = Arc::try_unwrap(source.content)
                        .unwrap_or_else(|content| (*content).clone());
                    (path.to_string_lossy().to_string(), content)
                })
                .collect(),
            abi: $value.abi.as_ref().map(|abi| {
                serde_json::to_string(abi)
                    .expect("Is result of local compilation and, thus, should be always valid")
            }),
            constructor_arguments: $value.constructor_args.map(|args| args.to_string()),
            match_type: match_type.into(),
        }
    }};
}

pub fn from_solidity_success(value: SoliditySuccess) -> Source {
    let source_type = match value.compiler_input.language.as_str() {
        "Solidity" => source::SourceType::Solidity,
        "Yul" => source::SourceType::Yul,
        _ => source::SourceType::Unspecified,
    };
    from_success!(value, source_type)
}

pub fn from_vyper_success(value: VyperSuccess) -> Source {
    from_success!(value, source::SourceType::Vyper)
}

pub fn from_sourcify_success(value: SourcifySuccess) -> Source {
    let match_type = match value.match_type {
        MatchType::Partial => source::MatchType::Partial,
        MatchType::Full => source::MatchType::Full,
    };

    Source {
        file_name: value.file_name,
        contract_name: value.contract_name,
        compiler_version: value.compiler_version,
        compiler_settings: value.compiler_settings,
        source_type: source::SourceType::Solidity.into(),
        source_files: value.sources,
        abi: Some(value.abi),
        constructor_arguments: value
            .constructor_arguments
            .map(|bytes| DisplayBytes::from(bytes).to_string()),
        match_type: match_type.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers_solc::{
        artifacts::{self, Libraries, Optimizer, Settings},
        CompilerInput, EvmVersion,
    };
    use pretty_assertions::assert_eq;
    use smart_contract_verifier::Version;
    use std::{collections::BTreeMap, str::FromStr};

    #[test]
    fn test_from_solidity_success() {
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
        let verification_success = SoliditySuccess {
            compiler_input: CompilerInput {
                language: "Solidity".to_string(),
                sources: BTreeMap::from([("file_name".into(), artifacts::Source::new("content"))]),
                settings: compiler_settings.clone(),
            },
            compiler_output: Default::default(),
            compiler_version: Version::from_str("v0.8.17+commit.8df45f5f").unwrap(),
            file_path: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            abi: Some(Default::default()),
            constructor_args: Some(DisplayBytes::from_str("0x123456").unwrap()),
            local_bytecode_parts: Default::default(),
            match_type: MatchType::Partial,
        };

        let result = from_solidity_success(verification_success);

        let expected = Source {
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            compiler_settings: serde_json::to_string(&compiler_settings).unwrap(),
            source_type: source::SourceType::Solidity.into(),
            source_files: BTreeMap::from([("file_name".into(), "content".into())]),
            constructor_arguments: Some("0x123456".into()),
            abi: Some(serde_json::to_string(&ethabi::Contract::default()).unwrap()),
            match_type: source::MatchType::Partial.into(),
        };

        assert_eq!(expected, result);
    }

    #[test]
    fn test_from_vyper_success() {
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
        let verification_success = VyperSuccess {
            compiler_input: CompilerInput {
                language: "Vyper".to_string(),
                sources: BTreeMap::from([("file_name".into(), artifacts::Source::new("content"))]),
                settings: compiler_settings.clone(),
            },
            compiler_output: Default::default(),
            compiler_version: Version::from_str("v0.3.9+commit.66b96705").unwrap(),
            file_path: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            abi: Some(Default::default()),
            constructor_args: Some(DisplayBytes::from_str("0x123456").unwrap()),
            local_bytecode_parts: Default::default(),
            match_type: MatchType::Partial,
        };

        let result = from_vyper_success(verification_success);

        let expected = Source {
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "v0.3.9+commit.66b96705".to_string(),
            compiler_settings: serde_json::to_string(&compiler_settings).unwrap(),
            source_type: source::SourceType::Vyper.into(),
            source_files: BTreeMap::from([("file_name".into(), "content".into())]),
            constructor_arguments: Some("0x123456".into()),
            abi: Some(serde_json::to_string(&ethabi::Contract::default()).unwrap()),
            match_type: source::MatchType::Partial.into(),
        };

        assert_eq!(expected, result);
    }

    #[test]
    fn test_from_sourcify_success() {
        let verification_success = SourcifySuccess {
            file_name: "file_name".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            evm_version: Some("london".to_string()),
            optimization: Some(true),
            optimization_runs: Some(200),
            constructor_arguments: Some(DisplayBytes::from_str("0x123456").unwrap().0),
            contract_name: "contract_name".to_string(),
            abi: "abi".to_string(),
            sources: BTreeMap::from([("file_name".into(), "content".into())]),
            contract_libraries: BTreeMap::from([("lib_name".into(), "lib_address".into())]),
            compiler_settings: "compiler_settings".to_string(),
            match_type: MatchType::Full,
        };
        let result = from_sourcify_success(verification_success);

        let expected = Source {
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            compiler_settings: "compiler_settings".to_string(),
            source_type: source::SourceType::Solidity.into(),
            source_files: BTreeMap::from([("file_name".into(), "content".into())]),
            constructor_arguments: Some("0x123456".into()),
            abi: Some("abi".to_string()),
            match_type: source::MatchType::Full.into(),
        };

        assert_eq!(expected, result);
    }
}
