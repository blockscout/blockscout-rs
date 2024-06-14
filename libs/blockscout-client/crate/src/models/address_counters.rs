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
pub struct AddressCounters {
    #[serde(rename = "transactions_count")]
    pub transactions_count: String,
    #[serde(rename = "token_transfers_count")]
    pub token_transfers_count: String,
    #[serde(rename = "gas_usage_count")]
    pub gas_usage_count: String,
    #[serde(rename = "validations_count")]
    pub validations_count: String,
}

impl AddressCounters {
    pub fn new(
        transactions_count: String,
        token_transfers_count: String,
        gas_usage_count: String,
        validations_count: String,
    ) -> AddressCounters {
        AddressCounters {
            transactions_count,
            token_transfers_count,
            gas_usage_count,
            validations_count,
        }
    }
}
