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

#[derive(derive_new::new, Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct SmartContractForList {
    #[serde(rename = "address")]
    pub address: models::AddressParam,
    #[serde(rename = "coin_balance")]
    pub coin_balance: Option<String>, // changed
    #[serde(rename = "compiler_version")]
    pub compiler_version: String,
    #[serde(rename = "language")]
    pub language: String,
    #[serde(rename = "has_constructor_args")]
    pub has_constructor_args: bool,
    #[serde(rename = "optimization_enabled")]
    pub optimization_enabled: bool,
    #[serde(rename = "transaction_count")]
    pub transaction_count: i32,
    #[serde(rename = "verified_at")]
    pub verified_at: String,
    #[serde(rename = "market_cap", skip_serializing_if = "Option::is_none")]
    pub market_cap: Option<String>, // changed
}
