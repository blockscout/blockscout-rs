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
pub struct TokenTransfer {
    #[serde(rename = "block_hash")]
    pub block_hash: String,
    #[serde(rename = "from")]
    pub from: models::AddressParam,
    #[serde(rename = "log_index")]
    pub log_index: String,
    #[serde(rename = "method")]
    pub method: String,
    #[serde(rename = "timestamp")]
    pub timestamp: String,
    #[serde(rename = "to")]
    pub to: models::AddressParam,
    #[serde(rename = "token")]
    pub token: models::TokenInfo,
    #[serde(rename = "total")]
    pub total: models::TokenTransferTotal,
    #[serde(rename = "transaction_hash")]
    pub transaction_hash: String,
    #[serde(rename = "type")]
    pub r#type: String,
}

impl TokenTransfer {
    pub fn new(block_hash: String, from: models::AddressParam, log_index: String, method: String, timestamp: String, to: models::AddressParam, token: models::TokenInfo, total: models::TokenTransferTotal, transaction_hash: String, r#type: String) -> TokenTransfer {
        TokenTransfer {
            block_hash,
            from,
            log_index,
            method,
            timestamp,
            to,
            token,
            total,
            transaction_hash,
            r#type,
        }
    }
}

