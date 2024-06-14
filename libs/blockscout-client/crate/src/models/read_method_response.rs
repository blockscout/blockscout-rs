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
pub struct ReadMethodResponse {
    #[serde(rename = "is_error")]
    pub is_error: bool,
    #[serde(rename = "result")]
    pub result: models::ReadMethodResponseResult,
}

impl ReadMethodResponse {
    pub fn new(is_error: bool, result: models::ReadMethodResponseResult) -> ReadMethodResponse {
        ReadMethodResponse { is_error, result }
    }
}
