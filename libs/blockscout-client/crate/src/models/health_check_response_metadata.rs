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
pub struct HealthCheckResponseMetadata {
    #[serde(rename = "latest_block", skip_serializing_if = "Option::is_none")]
    pub latest_block: Option<models::LatestBlock>,
    #[serde(rename = "healthy", skip_serializing_if = "Option::is_none")]
    pub healthy: Option<bool>,
}

impl HealthCheckResponseMetadata {
    pub fn new() -> HealthCheckResponseMetadata {
        HealthCheckResponseMetadata {
            latest_block: None,
            healthy: None,
        }
    }
}
