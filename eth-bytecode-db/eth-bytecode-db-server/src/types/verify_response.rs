use super::source::SourceWrapper;
use crate::proto;
use amplify::{From, Wrapper};
use eth_bytecode_db::verification;

#[derive(Wrapper, From, Clone, Debug, PartialEq)]
pub struct VerifyResponseWrapper(proto::VerifyResponse);

impl VerifyResponseWrapper {
    pub fn ok(verification_source: verification::Source) -> Self {
        proto::VerifyResponse {
            message: "OK".to_string(),
            status: proto::verify_response::Status::Success.into(),
            source: Some(SourceWrapper::from(verification_source).into_inner()),
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

    #[test]
    fn ok_verify_response() {
        let verification_source = verification::Source {
            file_name: "".to_string(),
            contract_name: "".to_string(),
            compiler_version: "".to_string(),
            compiler_settings: "".to_string(),
            source_type: verification::SourceType::Solidity,
            source_files: Default::default(),
            abi: None,
            constructor_arguments: None,
            match_type: verification::MatchType::Unknown,
            compilation_artifacts: None,
            creation_input_artifacts: None,
            deployed_bytecode_artifacts: None,
            raw_creation_input: vec![],
            raw_deployed_bytecode: vec![],
            creation_input_parts: vec![],
            deployed_bytecode_parts: vec![],
        };

        let expected = proto::VerifyResponse {
            message: "OK".to_string(),
            status: proto::verify_response::Status::Success.into(),
            source: Some(SourceWrapper::from(verification_source.clone()).into_inner()),
        };

        let response = VerifyResponseWrapper::ok(verification_source).into_inner();

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
