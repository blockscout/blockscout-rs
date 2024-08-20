use super::{
    artifacts::{CompilerInput, Interface, Interfaces, Settings},
    client::Client,
    types::Success,
};
use crate::{
    compiler::DetailedVersion,
    verifier::{ContractVerifier, Error},
};
use bytes::Bytes;
use foundry_compilers::{
    artifacts::{Source, Sources},
    EvmVersion,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRequest {
    pub deployed_bytecode: Bytes,
    pub creation_bytecode: Option<Bytes>,
    pub compiler_version: DetailedVersion,

    pub content: MultiFileContent,

    // Required for the metrics. Has no functional meaning.
    // In case if chain_id has not been provided, results in empty string.
    pub chain_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiFileContent {
    pub sources: BTreeMap<PathBuf, String>,
    pub interfaces: BTreeMap<PathBuf, String>,
    pub evm_version: Option<EvmVersion>,
}

impl TryFrom<MultiFileContent> for CompilerInput {
    type Error = Error;

    fn try_from(content: MultiFileContent) -> Result<Self, Self::Error> {
        let settings = Settings {
            evm_version: content.evm_version,
            ..Default::default()
        };

        let sources: Sources = content
            .sources
            .into_iter()
            .map(|(path, content)| (path, Source::new(content)))
            .collect();
        let interfaces = content
            .interfaces
            .into_iter()
            .map(|(path, content)| {
                Interface::try_new(path.as_path(), content).map(|interface| (path, interface))
            })
            .collect::<Result<Interfaces, _>>()
            .map_err(Error::Initialization)?;

        Ok(CompilerInput {
            language: "Vyper".to_string(),
            sources,
            interfaces,
            settings,
        })
    }
}

pub async fn verify(client: Arc<Client>, request: VerificationRequest) -> Result<Success, Error> {
    let compiler_input = CompilerInput::try_from(request.content)?;
    let verifier = ContractVerifier::new(
        client.compilers(),
        &request.compiler_version,
        request.creation_bytecode,
        request.deployed_bytecode,
        request.chain_id,
    )?;
    let result = verifier.verify(&compiler_input).await?;

    // If case of success, we allow middlewares to process success and only then return it to the caller;
    // Otherwise, we just return an error
    let success = Success::from((compiler_input, result));
    if let Some(middleware) = client.middleware() {
        middleware.call(&success).await;
    }

    Ok(success)
}
