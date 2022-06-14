#![allow(dead_code)]

use std::{collections::BTreeMap, fmt::Display};

use serde::{Deserialize, Serialize};

mod routes;
mod solidity;
mod sourcify;

pub use routes::AppConfig;
pub use sourcify::api::SourcifyApiClient;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct VerificationResponse {
    pub message: String,
    pub result: Option<VerificationResult>,
    pub status: VerificationStatus,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct VerificationResult {
    pub contract_name: String,
    pub compiler_version: String,
    pub evm_version: String,
    pub constructor_arguments: Option<String>,
    pub optimization: Option<bool>,
    pub optimization_runs: Option<usize>,
    pub contract_libraries: BTreeMap<String, String>,
    pub abi: String,
    pub sources: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub enum VerificationStatus {
    #[serde(rename = "0")]
    Ok,
    #[serde(rename = "1")]
    Failed,
}

impl VerificationResponse {
    pub fn ok(result: VerificationResult) -> Self {
        Self {
            message: "OK".to_string(),
            result: Some(result),
            status: VerificationStatus::Ok,
        }
    }

    pub fn err(message: impl Display) -> Self {
        Self {
            message: message.to_string(),
            result: None,
            status: VerificationStatus::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::parse::test_serialize_json_ok;
    use serde_json::json;

    #[test]
    fn parse_response() {
        test_serialize_json_ok(vec![
            (
                VerificationResponse::ok(VerificationResult {
                    contract_name: "contract_name".to_string(),
                    compiler_version: "compiler_version".to_string(),
                    evm_version: "evm_version".to_string(),
                    constructor_arguments: Some("constructor_arguments".to_string()),
                    optimization: Some(false),
                    optimization_runs: Some(200),
                    contract_libraries: BTreeMap::from([(
                        "some_library".into(),
                        "some_address".into(),
                    )]),
                    abi: "abi".to_string(),
                    sources: serde_json::from_str(
                        r#"{
                            "source.sol": "content"
                        }"#,
                    )
                    .unwrap(),
                }),
                json!({
                    "message": "OK",
                    "status": "0",
                    "result": {
                        "contract_name": "contract_name",
                        "compiler_version": "compiler_version",
                        "evm_version": "evm_version",
                        "constructor_arguments": "constructor_arguments",
                        "contract_libraries": {
                            "some_library": "some_address",
                        },
                        "optimization": false,
                        "optimization_runs": 200,
                        "abi": "abi",
                        "sources": {
                            "source.sol": "content",
                        },
                    },

                }),
            ),
            (
                VerificationResponse::err("Parse error"),
                json!({
                    "message": "Parse error",
                    "status": "1",
                    "result": null,
                }),
            ),
        ])
    }
}
