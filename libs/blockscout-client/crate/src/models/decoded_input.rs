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
pub struct DecodedInput {
    #[serde(rename = "method_call")]
    pub method_call: String,
    #[serde(rename = "method_id")]
    pub method_id: String,
    #[serde(rename = "parameters")]
    pub parameters: Vec<models::DecodedInputParameter>,
}

impl DecodedInput {
    pub fn new(
        method_call: String,
        method_id: String,
        parameters: Vec<models::DecodedInputParameter>,
    ) -> DecodedInput {
        DecodedInput {
            method_call,
            method_id,
            parameters,
        }
    }
}
