use super::{
    compiler::SolidityCompiler,
    contract_verifier::{ContractVerifier, Error, Success},
};
use crate::compilers::{Compilers, Version};
use bytes::Bytes;
use ethers_solc::{
    artifacts::{BytecodeHash, Libraries, Settings, SettingsMetadata, Source, Sources},
    CompilerInput, EvmVersion,
};
use semver::VersionReq;
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
    pub optimization_runs: Option<usize>,
    pub contract_libraries: Option<BTreeMap<String, String>>,
}

impl From<MultiFileContent> for CompilerInput {
    fn from(content: MultiFileContent) -> Self {
        let mut settings = Settings::default();
        settings.optimizer.enabled = Some(content.optimization_runs.is_some());
        settings.optimizer.runs = content.optimization_runs;
        if let Some(libs) = content.contract_libraries {
            // we have to know filename for library, but we don't know,
            // so we assume that every file MAY contains all libraries
            let libs = content
                .sources
                .iter()
                .map(|(filename, _)| (PathBuf::from(filename), libs.clone()))
                .collect();
            settings.libraries = Libraries { libs };
        }
        settings.evm_version = content.evm_version;

        let sources: Sources = content
            .sources
            .into_iter()
            .map(|(name, content)| (name, Source { content }))
            .collect();
        CompilerInput {
            language: "Solidity".to_string(),
            sources,
            settings,
        }
    }
}

pub async fn verify(
    compilers: Arc<Compilers<SolidityCompiler>>,
    request: VerificationRequest,
) -> Result<Success, Error> {
    let compiler_version = request.compiler_version;

    let verifier = ContractVerifier::new(
        compilers,
        &compiler_version,
        request.creation_bytecode,
        request.deployed_bytecode,
    )?;

    let mut compiler_input = CompilerInput::from(request.content);
    for metadata in settings_metadata(&compiler_version) {
        compiler_input.settings.metadata = metadata;
        let result = verifier.verify(&compiler_input).await;

        // If no matching contracts have been found, try the next settings metadata option
        if let Err(Error::NoMatchingContracts) = result {
            continue;
        }

        // Otherwise, verification either succeeded, or some uncorrectable error occurred
        return result;
    }

    // No contracts could be verified
    Err(Error::NoMatchingContracts)
}

/// Iterates through possible bytecode if required and creates
/// a corresponding variants of settings metadata for each of them.
///
/// Multi-file input type does not specify it explicitly, thus, we may
/// have to iterate through all possible options.
///
/// See "settings_metadata" (https://docs.soliditylang.org/en/v0.8.15/using-the-compiler.html?highlight=compiler%20input#input-description)
fn settings_metadata(compiler_version: &Version) -> Vec<Option<SettingsMetadata>> {
    // Options are sorted by their probability of occurring
    const BYTECODE_HASHES: [BytecodeHash; 3] =
        [BytecodeHash::Ipfs, BytecodeHash::None, BytecodeHash::Bzzr1];

    if VersionReq::parse("<0.6.0")
        .unwrap()
        .matches(compiler_version.version())
    {
        [None].into()
    } else {
        BYTECODE_HASHES
            .map(|hash| Some(SettingsMetadata::from(hash)))
            .into()
    }
}
