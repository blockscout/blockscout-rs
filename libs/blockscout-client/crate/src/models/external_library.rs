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
pub struct ExternalLibrary {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "address_hash")]
    pub address_hash: String,
}

impl ExternalLibrary {
    pub fn new(name: String, address_hash: String) -> ExternalLibrary {
        ExternalLibrary {
            name,
            address_hash,
        }
    }
}

