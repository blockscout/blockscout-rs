use super::compiler::VyperCompiler;
use crate::{
    compiler::{Compilers, Version},
    verifier::{ContractVerifier, Error, Success},
};
use bytes::Bytes;
use ethers_solc::{
    artifacts::{Settings, Source, Sources},
    CompilerInput, EvmVersion,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

pub struct VerificationRequest {
    pub deployed_bytecode: Bytes,
    pub creation_bytecode: Bytes,
    pub compiler_version: Version,

    pub content: MultiFileContent,
}

pub struct MultiFileContent {
    pub sources: BTreeMap<PathBuf, String>,
    pub evm_version: Option<EvmVersion>,
}

impl From<MultiFileContent> for CompilerInput {
    fn from(content: MultiFileContent) -> Self {
        let mut settings = Settings::default();
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

pub async fn verify(
    compilers: Arc<Compilers<VyperCompiler>>,
    request: VerificationRequest,
) -> Result<Success, Error> {
    let compiler_input = CompilerInput::from(request.content);
    let verifier = ContractVerifier::new(
        compilers,
        &request.compiler_version,
        request.creation_bytecode,
        request.deployed_bytecode,
    )?;

    verifier.verify(&compiler_input).await
}
