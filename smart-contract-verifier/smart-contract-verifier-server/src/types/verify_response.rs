use crate::proto::{
    verify_response::{ExtraData, Status},
    Source, VerifyResponse,
};
use serde::{Deserialize, Serialize};
use smart_contract_verifier::{SourcifySuccess, VerificationSuccess};
use std::{fmt::Display, mem, ops::Deref};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct VerifyResponseWrapper(VerifyResponse);

impl From<VerifyResponse> for VerifyResponseWrapper {
    fn from(inner: VerifyResponse) -> Self {
        Self(inner)
    }
}

impl Deref for VerifyResponseWrapper {
    type Target = VerifyResponse;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl VerifyResponseWrapper {
    pub fn into_inner(self) -> VerifyResponse {
        self.0
    }
}

pub trait VerifyResponseOk {
    fn result(self) -> (Source, ExtraData);
}

impl VerifyResponseOk for VerificationSuccess {
    fn result(mut self) -> (Source, ExtraData) {
        let local_bytecode_parts = mem::take(&mut self.local_bytecode_parts);
        let local_creation_input_parts = local_bytecode_parts
            .creation_tx_input_parts
            .into_iter()
            .map(|part| extra_data::bytecode_part::BytecodePartWrapper::from(part).into_inner())
            .collect();
        let local_deployed_bytecode_parts = local_bytecode_parts
            .deployed_bytecode_parts
            .into_iter()
            .map(|part| extra_data::bytecode_part::BytecodePartWrapper::from(part).into_inner())
            .collect();
        let extra_data = ExtraData {
            local_creation_input_parts,
            local_deployed_bytecode_parts,
        };

        let source = super::source::from_verification_success(self);

        (source, extra_data)
    }
}

impl VerifyResponseOk for SourcifySuccess {
    fn result(self) -> (Source, ExtraData) {
        let extra_data = ExtraData {
            local_creation_input_parts: vec![],
            local_deployed_bytecode_parts: vec![],
        };
        let source = super::source::from_sourcify_success(self);

        (source, extra_data)
    }
}

impl VerifyResponseWrapper {
    pub fn ok<T: VerifyResponseOk>(success: T) -> Self {
        let (source, extra_data) = success.result();
        VerifyResponse {
            message: "OK".to_string(),
            status: Status::Success.into(),
            source: Some(source),
            extra_data: Some(extra_data),
        }
        .into()
    }

    pub fn err(message: impl Display) -> Self {
        VerifyResponse {
            message: message.to_string(),
            status: Status::Failure.into(),
            source: None,
            extra_data: None,
        }
        .into()
    }
}

pub mod extra_data {
    pub mod bytecode_part {
        use crate::proto::verify_response::extra_data::BytecodePart;

        use blockscout_display_bytes::Bytes as DisplayBytes;
        use serde::{Deserialize, Serialize};
        use std::ops::Deref;

        #[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
        pub struct BytecodePartWrapper(BytecodePart);

        impl From<BytecodePart> for BytecodePartWrapper {
            fn from(inner: BytecodePart) -> Self {
                Self(inner)
            }
        }

        impl Deref for BytecodePartWrapper {
            type Target = BytecodePart;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl BytecodePartWrapper {
            pub fn into_inner(self) -> BytecodePart {
                self.0
            }
        }

        impl From<smart_contract_verifier::BytecodePart> for BytecodePartWrapper {
            fn from(value: smart_contract_verifier::BytecodePart) -> Self {
                let inner = match value {
                    smart_contract_verifier::BytecodePart::Main { raw } => BytecodePart {
                        r#type: "main".to_string(),
                        data: DisplayBytes::from(raw).to_string(),
                    },
                    smart_contract_verifier::BytecodePart::Metadata { raw, .. } => BytecodePart {
                        r#type: "meta".to_string(),
                        data: DisplayBytes::from(raw).to_string(),
                    },
                };
                inner.into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{extra_data::bytecode_part::BytecodePartWrapper, *};
    use crate::proto::verify_response::extra_data::BytecodePart;
    use blockscout_display_bytes::Bytes as DisplayBytes;
    use ethers_solc::CompilerInput;
    use pretty_assertions::assert_eq;
    use smart_contract_verifier::{MatchType, VerificationSuccess, Version};
    use std::str::FromStr;

    #[test]
    fn ok_verify_response() {
        let verification_success = VerificationSuccess {
            compiler_input: CompilerInput {
                language: "Solidity".to_string(),
                sources: Default::default(),
                settings: Default::default(),
            },
            compiler_output: Default::default(),
            compiler_version: Version::from_str("v0.8.17+commit.8df45f5f").unwrap(),
            file_path: "file_path".to_string(),
            contract_name: "contract_name".to_string(),
            abi: None,
            constructor_args: None,
            local_bytecode_parts: Default::default(),
            match_type: MatchType::Partial,
        };

        let response = VerifyResponseWrapper::ok(verification_success.clone()).into_inner();

        let expected = VerifyResponse {
            message: "OK".to_string(),
            status: Status::Success.into(),
            source: Some(super::super::source::from_verification_success(
                verification_success,
            )),
            extra_data: Some(ExtraData {
                local_creation_input_parts: vec![],
                local_deployed_bytecode_parts: vec![],
            }),
        };

        assert_eq!(expected, response);
    }

    #[test]
    fn err_verify_response() {
        let response = VerifyResponseWrapper::err("parse error").into_inner();
        let expected = VerifyResponse {
            message: "parse error".to_string(),
            status: Status::Failure.into(),
            source: None,
            extra_data: None,
        };
        assert_eq!(expected, response);
    }

    #[test]
    fn from_bytecode_parts() {
        // Main part
        let verifier_bytecode_part = smart_contract_verifier::BytecodePart::Main {
            raw: DisplayBytes::from_str("0x1234").unwrap().0,
        };
        let proto_bytecode_part = BytecodePartWrapper::from(verifier_bytecode_part).into_inner();
        let expected = BytecodePart {
            r#type: "main".to_string(),
            data: "0x1234".to_string(),
        };
        assert_eq!(expected, proto_bytecode_part);

        // Meta part
        let verifier_bytecode_part = smart_contract_verifier::BytecodePart::Metadata {
            raw: DisplayBytes::from_str("0x1234").unwrap().0,
            metadata: Default::default(),
        };
        let proto_bytecode_part = BytecodePartWrapper::from(verifier_bytecode_part).into_inner();
        let expected = BytecodePart {
            r#type: "meta".to_string(),
            data: "0x1234".to_string(),
        };
        assert_eq!(expected, proto_bytecode_part);
    }
}
