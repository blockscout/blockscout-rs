use serde::{Deserialize, Serialize};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::VerifyResponse;
use std::{fmt::Display, ops::Deref};

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

impl VerifyResponseWrapper {
    pub fn ok(result: verify_response::ResultWrapper) -> Self {
        VerifyResponse {
            message: "OK".to_string(),
            status: "0".to_string(),
            result: Some(result.into_inner()),
        }
        .into()
    }

    pub fn err(message: impl Display) -> Self {
        VerifyResponse {
            message: message.to_string(),
            status: "1".to_string(),
            result: None,
        }
        .into()
    }
}

pub mod verify_response {
    use std::ops::Deref;
    use smart_contract_verifier::VerificationSuccess;
    pub use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::verify_response::Result;
    use serde::{Serialize, Deserialize};

    #[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
    pub struct ResultWrapper(Result);

    impl From<Result> for ResultWrapper {
        fn from(inner: Result) -> Self {
            Self(inner)
        }
    }

    impl Deref for ResultWrapper {
        type Target = Result;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl ResultWrapper {
        pub fn into_inner(self) -> Result {
            self.0
        }
    }

    impl From<VerificationSuccess> for ResultWrapper {
        fn from(value: VerificationSuccess) -> Self {
            let compiler_input = value.compiler_input;
            let compiler_settings = serde_json::to_string(&compiler_input.settings).unwrap();

            let inner = Result {
                file_name: value.file_path,
                contract_name: value.contract_name,
                compiler_version: value.compiler_version.to_string(),
                evm_version: compiler_input
                    .settings
                    .evm_version
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "default".to_string()),
                sources: compiler_input
                    .sources
                    .into_iter()
                    .map(|(path, source)| (path.to_string_lossy().to_string(), source.content))
                    .collect(),
                optimization: compiler_input.settings.optimizer.enabled,
                optimization_runs: compiler_input.settings.optimizer.runs.map(|i| i as i32),
                contract_libraries: compiler_input
                    .settings
                    .libraries
                    .libs
                    .into_iter()
                    .flat_map(|(_path, libs)| libs)
                    .collect(),
                compiler_settings,
                constructor_arguments: value.constructor_args.map(|args| args.to_string()),
                abi: value.abi.as_ref().map(|abi| {
                    serde_json::to_string(abi)
                        .expect("Is result of local compilation and, thus, should be always valid")
                }),
                local_creation_input_parts: value
                    .local_bytecode_parts
                    .creation_tx_input_parts
                    .into_iter()
                    .map(|part| result::BytecodePartWrapper::from(part).into_inner())
                    .collect(),
                local_deployed_bytecode_parts: value
                    .local_bytecode_parts
                    .deployed_bytecode_parts
                    .into_iter()
                    .map(|part| result::BytecodePartWrapper::from(part).into_inner())
                    .collect(),
            };

            inner.into()
        }
    }

    pub mod result {
        use std::ops::Deref;
        pub use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::verify_response::result::BytecodePart;
        use serde::{Serialize, Deserialize};
        use blockscout_display_bytes::Bytes as DisplayBytes;

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
    use super::{
        verify_response::{
            result::{BytecodePart, BytecodePartWrapper},
            Result, ResultWrapper,
        },
        *,
    };
    use blockscout_display_bytes::Bytes as DisplayBytes;
    use ethers_solc::{
        artifacts::{Libraries, Optimizer, Settings, Source},
        CompilerInput, EvmVersion,
    };
    use pretty_assertions::assert_eq;
    use smart_contract_verifier::{VerificationSuccess, Version};
    use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::VerifyResponse;
    use std::{collections::BTreeMap, str::FromStr};

    #[test]
    fn from_verification_success() {
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
                    "path".into(),
                    Source {
                        content: "content".into(),
                    },
                )]),
                settings: compiler_settings.clone(),
            },
            compiler_output: Default::default(),
            compiler_version: Version::from_str("v0.8.17+commit.8df45f5f").unwrap(),
            file_path: "file_path".to_string(),
            contract_name: "contract_name".to_string(),
            abi: Some(Default::default()),
            constructor_args: Some(DisplayBytes::from_str("0x123456").unwrap()),
            local_bytecode_parts: Default::default(),
        };

        let result = ResultWrapper::from(verification_success).into_inner();

        let expected = Result {
            file_name: "file_path".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            sources: BTreeMap::from([("path".into(), "content".into())]),
            evm_version: "london".to_string(),
            optimization: Some(true),
            optimization_runs: Some(200),
            contract_libraries: BTreeMap::from([("lib_name".into(), "lib_address".into())]),
            compiler_settings: serde_json::to_string(&compiler_settings).unwrap(),
            constructor_arguments: Some("0x123456".into()),
            abi: Some(serde_json::to_string(&ethabi::Contract::default()).unwrap()),
            local_creation_input_parts: vec![],
            local_deployed_bytecode_parts: vec![],
        };

        assert_eq!(expected, result);
    }

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
        };
        let result = ResultWrapper::from(verification_success);

        let response = VerifyResponseWrapper::ok(result.clone()).into_inner();

        let expected = VerifyResponse {
            message: "OK".to_string(),
            status: "0".to_string(),
            result: Some(result.into_inner()),
        };

        assert_eq!(expected, response);
    }

    #[test]
    fn err_verify_response() {
        let response = VerifyResponseWrapper::err("parse error").into_inner();
        let expected = VerifyResponse {
            message: "parse error".to_string(),
            status: "1".to_string(),
            result: None,
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
