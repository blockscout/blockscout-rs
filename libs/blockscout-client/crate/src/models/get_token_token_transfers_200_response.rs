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
pub struct GetTokenTokenTransfers200Response {
    #[serde(rename = "items")]
    pub items: Vec<models::TokenTransfer>,
    #[serde(rename = "next_page_params")]
    pub next_page_params: serde_json::Value,
}

impl GetTokenTokenTransfers200Response {
    pub fn new(items: Vec<models::TokenTransfer>, next_page_params: serde_json::Value) -> GetTokenTokenTransfers200Response {
        GetTokenTokenTransfers200Response {
            items,
            next_page_params,
        }
    }
}

