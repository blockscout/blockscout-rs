/*
 * BlockScout API
 *
 * API for BlockScout web app
 *
 * The version of the OpenAPI document: 1.0.0
 * Contact: you@your-company.com
 * Generated by: https://openapi-generator.tech
 */

use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContractSource {
    #[serde(rename = "file_path", skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(rename = "source_code", skip_serializing_if = "Option::is_none")]
    pub source_code: Option<String>,
}

impl ContractSource {
    pub fn new() -> ContractSource {
        ContractSource {
            file_path: None,
            source_code: None,
        }
    }
}

