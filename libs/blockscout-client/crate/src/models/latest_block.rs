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
pub struct LatestBlock {
    #[serde(rename = "cache", skip_serializing_if = "Option::is_none")]
    pub cache: Option<models::LatestBlockCache>,
    #[serde(rename = "db", skip_serializing_if = "Option::is_none")]
    pub db: Option<models::LatestBlockDb>,
}

impl LatestBlock {
    pub fn new() -> LatestBlock {
        LatestBlock {
            cache: None,
            db: None,
        }
    }
}
