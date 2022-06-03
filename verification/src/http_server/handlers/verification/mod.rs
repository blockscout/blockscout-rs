#![allow(dead_code)]

use std::{collections::HashMap, fmt::Display};

use serde::{Deserialize, Serialize};

pub mod routes;
pub mod solidity;
pub mod sourcify;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ContractLibrary {
    pub lib_name: String,
    pub lib_address: String,
}

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
    pub contract_libraries: Option<Vec<ContractLibrary>>,
    pub abi: String,
    pub sources: HashMap<String, String>,
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
            message: format!("{}", message),
            result: None,
            status: VerificationStatus::Failed,
        }
    }
}
