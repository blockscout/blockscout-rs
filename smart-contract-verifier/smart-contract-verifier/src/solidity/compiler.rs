use super::solc_cli;
use crate::compiler::{EvmCompiler, Version};
use ethers_solc::{error::SolcError, CompilerOutput, Solc};
use std::path::Path;

#[derive(Default)]
pub struct SolidityCompiler {}

impl SolidityCompiler {
    pub fn new() -> Self {
        SolidityCompiler {}
    }
}

#[async_trait::async_trait]
impl EvmCompiler for SolidityCompiler {
    type CompilerInput = foundry_compilers::CompilerInput;

    async fn compile(
        &self,
        path: &Path,
        ver: &Version,
        input: &Self::CompilerInput,
    ) -> Result<(serde_json::Value, CompilerOutput), SolcError> {
        if ver.version() < &semver::Version::new(0, 4, 11) {
            let output = solc_cli::compile_using_cli(path, input).await?;
            Ok((serde_json::to_value(&output).unwrap(), output))
        } else {
            let raw = Solc::from(path).async_compile_output(input).await?;
            Ok((serde_json::from_slice(&raw)?, serde_json::from_slice(&raw)?))
        }
    }

    fn contains_metadata_hash(version: &Version, input: &Self::CompilerInput) -> bool {
        // Before v0.4.7 there was no metadata hash included
        if version.version() < &semver::Version::new(0, 4, 7) {
            return false;
        }

        // Starting from v0.4.7 and before 0.6.0 it was impossible to disable appending metadata hash to the bytecode
        if version.version() < &semver::Version::new(0, 6, 0) {
            return true;
        }

        if let Some(metadata) = &input.settings.metadata {
            // If cbor metadata is missed, metadata hash bytecode part is also absent
            if let Some(false) = metadata.cbor_metadata {
                return false;
            }

            if let Some(foundry_compilers::artifacts::BytecodeHash::None) = metadata.bytecode_hash {
                return false;
            }
        }

        true
    }
}
