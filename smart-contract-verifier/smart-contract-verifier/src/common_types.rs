/// The enum representing how provided bytecode corresponds
/// to the local result of source codes compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchType {
    Partial,
    Full,
}

impl From<sourcify::MatchType> for MatchType {
    fn from(value: sourcify::MatchType) -> Self {
        match value {
            sourcify::MatchType::Full => MatchType::Full,
            sourcify::MatchType::Partial => MatchType::Partial,
        }
    }
}

macro_rules! from_success {
    ( $value:expr, $source_type:expr, $extract_source_files:expr ) => {{
        let compiler_input = $value.compiler_input;
        let compiler_settings = serde_json::to_string(&compiler_input.settings)
            .expect("Is result of local compilation and, thus, should be always valid");

        let match_type = match $value.match_type {
            $crate::MatchType::Partial => source::MatchType::Partial,
            $crate::MatchType::Full => source::MatchType::Full,
        };

        Source {
            file_name: $value.file_path,
            contract_name: $value.contract_name,
            compiler_version: $value.compiler_version.to_string(),
            compiler_settings,
            source_type: $source_type.into(),
            source_files: $extract_source_files(compiler_input),
            abi: $value.abi.as_ref().map(|abi| {
                serde_json::to_string(abi)
                    .expect("Is result of local compilation and, thus, should be always valid")
            }),
            constructor_arguments: $value.constructor_args.map(|args| args.to_string()),
            match_type: match_type.into(),
            compilation_artifacts: Some(
                serde_json::to_string(&$value.compilation_artifacts).unwrap(),
            ),
            creation_input_artifacts: Some(
                serde_json::to_string(&$value.creation_input_artifacts).unwrap(),
            ),
            deployed_bytecode_artifacts: Some(
                serde_json::to_string(&$value.deployed_bytecode_artifacts).unwrap(),
            ),
        }
    }};
}
pub(crate) use from_success;
