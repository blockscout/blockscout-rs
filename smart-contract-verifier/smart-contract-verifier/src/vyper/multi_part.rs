use super::client::Client;
use crate::{
    compiler::Version,
    verifier::{ContractVerifier, Error, Success},
};
use bytes::Bytes;
use ethers_solc::{
    artifacts::{Settings, Source, Sources},
    CompilerInput, EvmVersion,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRequest {
    pub deployed_bytecode: Bytes,
    pub creation_bytecode: Option<Bytes>,
    pub compiler_version: Version,

    pub content: MultiFileContent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiFileContent {
    pub sources: BTreeMap<PathBuf, String>,
    pub evm_version: Option<EvmVersion>,
}

impl From<MultiFileContent> for CompilerInput {
    fn from(content: MultiFileContent) -> Self {
        let mut settings = Settings::default();
        settings.optimizer.enabled = None;
        settings.optimizer.runs = None;
        if let Some(version) = content.evm_version {
            settings.evm_version = Some(version);
        } else {
            // default evm version for vyper
            settings.evm_version = Some(EvmVersion::Istanbul)
        };

        let sources: Sources = content
            .sources
            .into_iter()
            .map(|(name, content)| (name, Source { content }))
            .collect();
        CompilerInput {
            language: "Vyper".to_string(),
            sources,
            settings,
        }
    }
}

pub async fn verify(client: Arc<Client>, request: VerificationRequest) -> Result<Success, Error> {
    let compiler_input = CompilerInput::from(request.content);
    let verifier = ContractVerifier::new(
        client.compilers(),
        &request.compiler_version,
        request.creation_bytecode,
        request.deployed_bytecode,
    )?;

    // If case of success, we allow middlewares to process success and only then return it to the caller;
    // Otherwise, we just return an error
    let success = verifier.verify(&compiler_input).await?;
    if let Some(middleware) = client.middleware() {
        middleware.call(&success).await;
    }
    Ok(success)
}
