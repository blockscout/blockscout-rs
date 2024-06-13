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
pub struct SmartContract {
    #[serde(
        rename = "verified_twin_address_hash",
        skip_serializing_if = "Option::is_none"
    )]
    pub verified_twin_address_hash: Option<String>,
    #[serde(rename = "is_verified", skip_serializing_if = "Option::is_none")]
    pub is_verified: Option<bool>,
    #[serde(
        rename = "is_changed_bytecode",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_changed_bytecode: Option<bool>,
    #[serde(
        rename = "is_partially_verified",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_partially_verified: Option<bool>,
    #[serde(rename = "is_fully_verified", skip_serializing_if = "Option::is_none")]
    pub is_fully_verified: Option<bool>,
    #[serde(
        rename = "is_verified_via_sourcify",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_verified_via_sourcify: Option<bool>,
    #[serde(
        rename = "is_verified_via_eth_bytecode_db",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_verified_via_eth_bytecode_db: Option<bool>,
    #[serde(rename = "is_vyper_contract", skip_serializing_if = "Option::is_none")]
    pub is_vyper_contract: Option<bool>,
    #[serde(rename = "is_self_destructed", skip_serializing_if = "Option::is_none")]
    pub is_self_destructed: Option<bool>,
    #[serde(
        rename = "can_be_visualized_via_sol2uml",
        skip_serializing_if = "Option::is_none"
    )]
    pub can_be_visualized_via_sol2uml: Option<bool>,
    #[serde(
        rename = "minimal_proxy_address_hash",
        skip_serializing_if = "Option::is_none"
    )]
    pub minimal_proxy_address_hash: Option<String>,
    #[serde(rename = "sourcify_repo_url", skip_serializing_if = "Option::is_none")]
    pub sourcify_repo_url: Option<String>,
    #[serde(rename = "name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(
        rename = "optimization_enabled",
        skip_serializing_if = "Option::is_none"
    )]
    pub optimization_enabled: Option<bool>,
    #[serde(rename = "optimizations_runs", skip_serializing_if = "Option::is_none")]
    pub optimizations_runs: Option<i32>,
    #[serde(rename = "compiler_version", skip_serializing_if = "Option::is_none")]
    pub compiler_version: Option<String>,
    #[serde(rename = "evm_version", skip_serializing_if = "Option::is_none")]
    pub evm_version: Option<String>,
    #[serde(rename = "verified_at", skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
    #[serde(rename = "abi", skip_serializing_if = "Option::is_none")]
    pub abi: Option<serde_json::Value>, // changed
    #[serde(rename = "source_code", skip_serializing_if = "Option::is_none")]
    pub source_code: Option<String>,
    #[serde(rename = "file_path", skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(rename = "compiler_settings", skip_serializing_if = "Option::is_none")]
    pub compiler_settings: Option<serde_json::Value>,
    #[serde(rename = "constructor_args", skip_serializing_if = "Option::is_none")]
    pub constructor_args: Option<String>,
    #[serde(rename = "additional_sources", skip_serializing_if = "Option::is_none")]
    pub additional_sources: Option<Vec<models::ContractSource>>,
    #[serde(
        rename = "decoded_constructor_args",
        skip_serializing_if = "Option::is_none"
    )]
    pub decoded_constructor_args: Option<Vec<serde_json::Value>>,
    #[serde(rename = "deployed_bytecode", skip_serializing_if = "Option::is_none")]
    pub deployed_bytecode: Option<String>,
    #[serde(rename = "creation_bytecode", skip_serializing_if = "Option::is_none")]
    pub creation_bytecode: Option<String>,
    #[serde(rename = "external_libraries", skip_serializing_if = "Option::is_none")]
    pub external_libraries: Option<Vec<models::ExternalLibrary>>,
    #[serde(rename = "language", skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}