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
pub struct IndexingStatus {
    #[serde(rename = "finished_indexing")]
    pub finished_indexing: bool,
    #[serde(rename = "finished_indexing_blocks")]
    pub finished_indexing_blocks: bool,
    #[serde(rename = "indexed_blocks_ratio")]
    pub indexed_blocks_ratio: Option<String>, // changed
    #[serde(rename = "indexed_internal_transactions_ratio")]
    pub indexed_internal_transactions_ratio: Option<String>, // changed
}
