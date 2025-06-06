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
pub struct LatestBlockDb {
    #[serde(rename = "timestamp", skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(rename = "number", skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
}

impl LatestBlockDb {
    pub fn new() -> LatestBlockDb {
        LatestBlockDb {
            timestamp: None,
            number: None,
        }
    }
}
