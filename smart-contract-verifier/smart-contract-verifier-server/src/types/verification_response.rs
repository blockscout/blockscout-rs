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
            status: 0,
            result: Some(result.into_inner()),
        }
        .into()
    }

    pub fn err(message: impl Display) -> Self {
        VerifyResponse {
            message: message.to_string(),
            status: 1,
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
                optimizations: compiler_input.settings.optimizer.enabled,
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
                    smart_contract_verifier::BytecodePart::Metadata { metadata_raw, .. } => {
                        BytecodePart {
                            r#type: "meta".to_string(),
                            data: DisplayBytes::from(metadata_raw).to_string(),
                        }
                    }
                };
                inner.into()
            }
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::tests::parse::test_serialize_json_ok;
//     use serde_json::json;
//     use std::str::FromStr;
//
//     #[test]
//     fn parse_response() {
//         test_serialize_json_ok(vec![
//             (
//                 VerificationResponse::ok(VerificationResult {
//                     file_name: "File.sol".to_string(),
//                     contract_name: "contract_name".to_string(),
//                     compiler_version: "compiler_version".to_string(),
//                     evm_version: "evm_version".to_string(),
//                     constructor_arguments: Some(DisplayBytes::from([0xca, 0xfe])),
//                     optimization: Some(false),
//                     optimization_runs: Some(200),
//                     contract_libraries: BTreeMap::from([(
//                         "some_library".into(),
//                         "some_address".into(),
//                     )]),
//                     abi: Some("abi".to_string()),
//                     sources: serde_json::from_str(
//                         r#"{
//                             "source.sol": "content"
//                         }"#,
//                     )
//                         .unwrap(),
//                     compiler_settings: "compiler_settings".into(),
//                     local_creation_input_parts: Some(vec![
//                         BytecodePart::Main {
//                             data: DisplayBytes::from_str("0x1234").unwrap(),
//                         },
//                         BytecodePart::Meta {
//                             data: DisplayBytes::from_str("0xcafe").unwrap(),
//                         },
//                     ]),
//                     local_deployed_bytecode_parts: Some(vec![]),
//                 }),
//                 json!({
//                     "message": "OK",
//                     "status": "0",
//                     "result": {
//                         "file_name": "File.sol",
//                         "contract_name": "contract_name",
//                         "compiler_version": "compiler_version",
//                         "evm_version": "evm_version",
//                         "constructor_arguments": "0xcafe",
//                         "contract_libraries": {
//                             "some_library": "some_address",
//                         },
//                         "optimization": false,
//                         "optimization_runs": 200,
//                         "abi": "abi",
//                         "compiler_settings": "compiler_settings",
//                         "sources": {
//                             "source.sol": "content",
//                         },
//                         "local_creation_input_parts": [
//                             { "type": "main", "data": "0x1234" },
//                             { "type": "meta", "data": "0xcafe" }
//                         ],
//                         "local_deployed_bytecode_parts": []
//                     },
//                 }),
//             ),
//             (
//                 VerificationResponse::err("Parse error"),
//                 json!({
//                     "message": "Parse error",
//                     "status": "1",
//                     "result": null,
//                 }),
//             ),
//         ])
//     }
// }
