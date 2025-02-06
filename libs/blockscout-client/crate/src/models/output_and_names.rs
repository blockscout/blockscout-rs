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
pub struct OutputAndNames {
    #[serde(rename = "output")]
    pub output: Vec<String>,
    #[serde(rename = "names")]
    pub names: Vec<String>,
}

impl OutputAndNames {
    pub fn new(output: Vec<String>, names: Vec<String>) -> OutputAndNames {
        OutputAndNames {
            output,
            names,
        }
    }
}

