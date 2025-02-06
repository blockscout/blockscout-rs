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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryReadMethod200ResponseInner {
    ReadMethodResponse(models::ReadMethodResponse),
}

impl Default for QueryReadMethod200ResponseInner {
    fn default() -> Self {
        Self::ReadMethodResponse(Default::default())
    }
}

