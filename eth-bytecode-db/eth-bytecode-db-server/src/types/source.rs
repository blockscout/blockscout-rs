use crate::{
    proto,
    types::{MatchTypeWrapper, SourceTypeWrapper},
};
use amplify::{From, Wrapper};
use eth_bytecode_db::{search, verification};

#[derive(Wrapper, From, Clone, Debug, PartialEq)]
pub struct SourceWrapper(proto::Source);

impl From<verification::Source> for SourceWrapper {
    fn from(value: verification::Source) -> Self {
        let source_type = SourceTypeWrapper::from(value.source_type).into_inner();
        let match_type = MatchTypeWrapper::from(value.match_type).into_inner();
        proto::Source {
            file_name: value.file_name,
            contract_name: value.contract_name,
            compiler_version: value.compiler_version,
            compiler_settings: value.compiler_settings,
            source_type: source_type.into(),
            source_files: value.source_files,
            abi: value.abi,
            constructor_arguments: value.constructor_arguments,
            match_type: match_type.into(),
        }
        .into()
    }
}

impl From<search::MatchContract> for SourceWrapper {
    fn from(value: search::MatchContract) -> Self {
        let source_type = SourceTypeWrapper::from(value.source_type).into_inner();
        let match_type = MatchTypeWrapper::from(value.match_type).into_inner();
        proto::Source {
            file_name: value.file_name,
            contract_name: value.contract_name,
            compiler_version: value.compiler_version,
            compiler_settings: value.compiler_settings,
            source_type: source_type.into(),
            source_files: value.source_files,
            abi: value.abi,
            constructor_arguments: value.constructor_arguments,
            match_type: match_type.into(),
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn from_verification_source_to_proto_source() {
        let verification_source = verification::Source {
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            compiler_settings: "compiler_settings".to_string(),
            source_type: verification::SourceType::Solidity,
            source_files: BTreeMap::from([("source".into(), "content".into())]),
            abi: Some("abi".into()),
            constructor_arguments: Some("args".into()),
            match_type: verification::MatchType::Partial,
            raw_creation_input: vec![0u8, 1u8, 2u8, 3u8, 4u8],
            raw_deployed_bytecode: vec![5u8, 6u8, 7u8, 8u8],
            creation_input_parts: vec![
                verification::BytecodePart::Main {
                    data: vec![0u8, 1u8],
                },
                verification::BytecodePart::Meta {
                    data: vec![3u8, 4u8],
                },
            ],
            deployed_bytecode_parts: vec![
                verification::BytecodePart::Main {
                    data: vec![5u8, 6u8],
                },
                verification::BytecodePart::Meta {
                    data: vec![7u8, 8u8],
                },
            ],
        };

        let expected = proto::Source {
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            compiler_settings: "compiler_settings".to_string(),
            source_type: proto::source::SourceType::Solidity.into(),
            source_files: BTreeMap::from([("source".into(), "content".into())]),
            abi: Some("abi".into()),
            constructor_arguments: Some("args".into()),
            match_type: proto::source::MatchType::Partial.into(),
        };

        let result = SourceWrapper::from(verification_source).into_inner();
        assert_eq!(expected, result);
    }

    #[test]
    fn from_search_source_to_proto_source() {
        let search_source = search::MatchContract {
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            compiler_settings: "compiler_settings".to_string(),
            source_type: verification::SourceType::Solidity,
            source_files: BTreeMap::from([("source".into(), "content".into())]),
            abi: Some("abi".into()),
            constructor_arguments: Some("args".into()),
            match_type: verification::MatchType::Partial,
            raw_creation_input: vec![0u8, 1u8, 2u8, 3u8, 4u8],
            raw_deployed_bytecode: vec![5u8, 6u8, 7u8, 8u8],
        };

        let expected = proto::Source {
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            compiler_settings: "compiler_settings".to_string(),
            source_type: proto::source::SourceType::Solidity.into(),
            source_files: BTreeMap::from([("source".into(), "content".into())]),
            abi: Some("abi".into()),
            constructor_arguments: Some("args".into()),
            match_type: proto::source::MatchType::Partial.into(),
        };

        let result = SourceWrapper::from(search_source).into_inner();
        assert_eq!(expected, result);
    }
}
