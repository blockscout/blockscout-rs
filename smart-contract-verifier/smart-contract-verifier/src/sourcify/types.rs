// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::MatchType;
use bytes::Bytes;
use serde::Deserialize;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Success {
    pub file_name: String,
    pub contract_name: String,
    pub compiler_version: String,
    pub evm_version: Option<String>,
    pub optimization: Option<bool>,
    pub optimization_runs: Option<usize>,
    pub constructor_arguments: Option<Bytes>,
    pub contract_libraries: BTreeMap<String, String>,
    pub abi: String,
    pub sources: BTreeMap<String, String>,
    pub compiler_settings: String,
    pub match_type: MatchType,
}

impl Success {
    /// Builds a [`Success`] from the parts of a Sourcify verified contract:
    /// the contract metadata, its sources, optional constructor arguments and
    /// the match type.
    fn from_sourcify_parts(
        raw_metadata: serde_json::Value,
        sources: BTreeMap<String, String>,
        constructor_arguments: Option<Bytes>,
        match_type: sourcify::MatchType,
    ) -> Result<Self, Error> {
        let metadata: foundry_compilers::artifacts::Metadata =
            serde_json::from_value(raw_metadata.clone()).map_err(|err| {
                tracing::error!(target: "sourcify", "returned metadata cannot be parsed: {err}");
                Error::Internal(anyhow::anyhow!(
                    "error occurred when parsing sourcify response"
                ))
            })?;

        let (compiler_settings, abi) = {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct CustomOutput {
                abi: serde_json::Value,
            }

            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct CustomMetadata {
                settings: serde_json::Value,
                output: CustomOutput,
            }

            let metadata: CustomMetadata = serde_json::from_value(raw_metadata)
                .expect("metadata has already been parsed successfully");

            let abi = metadata.output.abi;

            let mut compiler_settings = metadata
                .settings
                .as_object()
                .expect("metadata has been parsed successfully and 'settings' must be an object")
                .clone();
            compiler_settings.remove("compilationTarget");

            (compiler_settings, abi)
        };

        let evm_version = compiler_settings
            .get("evmVersion")
            .and_then(|value| value.as_str().map(|value| value.to_string()));

        let (file_name, contract_name) = metadata.settings.compilation_target.into_iter()
            .next().ok_or_else(|| {
            tracing::error!(target: "sourcify", "returned metadata does not contain any compilation target");
            Error::Internal(anyhow::anyhow!("error occurred when parsing sourcify response"))
        })?;

        Ok(Success {
            file_name,
            contract_name,
            compiler_version: metadata.compiler.version,
            evm_version,
            optimization: metadata.settings.optimizer.enabled,
            optimization_runs: metadata.settings.optimizer.runs,
            constructor_arguments,
            contract_libraries: metadata.settings.libraries,
            abi: serde_json::to_string(&abi).unwrap(),
            sources,
            compiler_settings: serde_json::to_string(&compiler_settings).unwrap(),
            match_type: MatchType::from(match_type),
        })
    }
}

impl TryFrom<sourcify::VerifiedContract> for Success {
    type Error = Error;

    fn try_from(value: sourcify::VerifiedContract) -> Result<Self, Self::Error> {
        Self::from_sourcify_parts(
            value.metadata,
            value.sources,
            value.constructor_arguments,
            value.match_type,
        )
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0:#}")]
    Internal(anyhow::Error),
    #[error("{0:#}")]
    BadRequest(anyhow::Error),
    #[error("verification error: {0}")]
    Verification(String),
    #[error("validation error: {0}")]
    Validation(String),
}
