use super::solc_cli;
use crate::compiler::{self, DetailedVersion, EvmCompiler};
use ethers_solc::{error::SolcError, CompilerOutput, Solc};
use foundry_compilers::artifacts::output_selection::OutputSelection;
use std::path::Path;

#[derive(Default)]
pub struct SolidityCompiler {}

impl SolidityCompiler {
    pub fn new() -> Self {
        SolidityCompiler {}
    }
}

impl compiler::CompilerInput for foundry_compilers::CompilerInput {
    fn modify(mut self) -> Self {
        // TODO: could we update some other field to avoid copying strings?
        self.sources.iter_mut().for_each(|(_file, source)| {
            let mut modified_content = source.content.as_ref().clone();
            modified_content.push(' ');
            source.content = std::sync::Arc::new(modified_content);
        });
        self
    }

    fn normalize_output_selection(&mut self, _version: &DetailedVersion) {
        self.settings.output_selection = OutputSelection::complete_output_selection();
    }
}

#[async_trait::async_trait]
impl EvmCompiler for SolidityCompiler {
    type CompilerInput = foundry_compilers::CompilerInput;

    async fn compile(
        &self,
        path: &Path,
        ver: &DetailedVersion,
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
}
