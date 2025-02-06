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
pub struct GetTransactionInternalTxs200Response {
    #[serde(rename = "items")]
    pub items: Vec<models::InternalTransaction>,
    #[serde(rename = "next_page_params")]
    pub next_page_params: serde_json::Value,
}

impl GetTransactionInternalTxs200Response {
    pub fn new(items: Vec<models::InternalTransaction>, next_page_params: serde_json::Value) -> GetTransactionInternalTxs200Response {
        GetTransactionInternalTxs200Response {
            items,
            next_page_params,
        }
    }
}

