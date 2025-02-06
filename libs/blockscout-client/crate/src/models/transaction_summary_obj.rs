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
pub struct TransactionSummaryObj {
    #[serde(rename = "summaries", skip_serializing_if = "Option::is_none")]
    pub summaries: Option<Vec<models::Summary>>,
}

impl TransactionSummaryObj {
    pub fn new() -> TransactionSummaryObj {
        TransactionSummaryObj {
            summaries: None,
        }
    }
}

