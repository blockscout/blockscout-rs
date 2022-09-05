use std::{collections::BTreeMap, path::PathBuf, str::FromStr};

use ethers_solc::{
    artifacts::{Settings, Source, Sources},
    CompilerInput, EvmVersion,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct VyperVerificationRequest {
    pub deployed_bytecode: String,
    pub creation_bytecode: String,
    pub compiler_version: String,
    pub sources: BTreeMap<PathBuf, String>,
    pub evm_version: Option<String>,
}

impl TryFrom<VyperVerificationRequest> for CompilerInput {
    type Error = anyhow::Error;

    fn try_from(request: VyperVerificationRequest) -> Result<Self, Self::Error> {
        let mut settings = Settings::default();
        settings.optimizer.enabled = None;
        settings.optimizer.runs = None;
        if let Some(version) = request.evm_version {
            settings.evm_version =
                Some(EvmVersion::from_str(&version).map_err(anyhow::Error::msg)?);
        } else {
            // default evm version for vyper
            settings.evm_version = Some(EvmVersion::Istanbul)
        };
        let sources: Sources = request
            .sources
            .into_iter()
            .map(|(name, content)| (name, Source { content }))
            .collect();
        Ok(CompilerInput {
            language: "Vyper".to_string(),
            sources,
            settings,
        })
    }
}
