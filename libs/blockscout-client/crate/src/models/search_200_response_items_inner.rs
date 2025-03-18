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
pub enum Search200ResponseItemsInner {
    SearchResultToken(models::SearchResultToken),
    SearchResultAddressOrContract(models::SearchResultAddressOrContract),
    SearchResultBlock(models::SearchResultBlock),
    SearchResultTransaction(models::SearchResultTransaction),
}

impl Default for Search200ResponseItemsInner {
    fn default() -> Self {
        Self::SearchResultToken(Default::default())
    }
}
