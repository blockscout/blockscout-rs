use super::enums::{MatchTypeWrapper, SourceTypeWrapper};
use crate::proto;
use amplify::{From, Wrapper};
use eth_bytecode_db::verification;

#[derive(Wrapper, From, Clone, Debug, PartialEq)]
pub struct VerifyResponseWrapper(proto::VerifyResponse);

impl VerifyResponseWrapper {
    pub fn ok(verification_source: verification::Source) -> Self {
        let source_type = SourceTypeWrapper::from(verification_source.source_type).into_inner();
        let match_type = MatchTypeWrapper::from(verification_source.match_type).into_inner();
        let proto_source = proto::Source {
            file_name: verification_source.file_name,
            contract_name: verification_source.contract_name,
            compiler_version: verification_source.compiler_version,
            compiler_settings: verification_source.compiler_settings,
            source_type: source_type.into(),
            source_files: verification_source.source_files,
            abi: verification_source.abi,
            constructor_arguments: verification_source.constructor_arguments,
            match_type: match_type.into(),
        };

        proto::VerifyResponse {
            message: "OK".to_string(),
            status: proto::verify_response::Status::Success.into(),
            source: Some(proto_source),
        }
        .into()
    }

    pub fn err(message: String) -> Self {
        proto::VerifyResponse {
            message,
            status: proto::verify_response::Status::Failure.into(),
            source: None,
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn ok_verify_response() {
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

        let response = VerifyResponseWrapper::ok(verification_source).into_inner();

        let expected = proto::VerifyResponse {
            message: "OK".to_string(),
            status: proto::verify_response::Status::Success.into(),
            source: Some(proto::Source {
                file_name: "file_name".to_string(),
                contract_name: "contract_name".to_string(),
                compiler_version: "compiler_version".to_string(),
                compiler_settings: "compiler_settings".to_string(),
                source_type: proto::source::SourceType::Solidity.into(),
                source_files: BTreeMap::from([("source".into(), "content".into())]),
                abi: Some("abi".into()),
                constructor_arguments: Some("args".into()),
                match_type: proto::source::MatchType::Partial.into(),
            }),
        };

        assert_eq!(expected, response);
    }

    #[test]
    fn err_verify_response() {
        let response = VerifyResponseWrapper::err("parse error".into()).into_inner();
        let expected = proto::VerifyResponse {
            message: "parse error".to_string(),
            status: proto::verify_response::Status::Failure.into(),
            source: None,
        };
        assert_eq!(expected, response);
    }
}
