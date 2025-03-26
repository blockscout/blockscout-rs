pub use super::vyper_compiler_input::VyperInput;
use crate::{
    verify_new::{evm_compilers, Error},
    DetailedVersion, Language, Version,
};
use anyhow::Context;
use async_trait::async_trait;
use foundry_compilers_new::artifacts::output_selection::OutputSelection;
use serde_json::Value;
use std::{collections::BTreeMap, path::Path, sync::Arc};

impl evm_compilers::CompilerInput for VyperInput {
    // Starting from pre-release versions of 0.4.0 interfaces are missing from input standard-json.
    // Due to that, we cannot specify output selection for all files (via "*" wildcard),
    // as some of them may be interfaces, which should not be compiled.
    // Thus, we start specifying required outputs only for those files
    // that already exists in the provided output_selection.
    fn normalize_output_selection(&mut self, version: &semver::Version) {
        // v0.3.10 was the latest release prior to v0.4.0 pre-releases
        if version > &semver::Version::new(0, 3, 10) {
            let default_output_selection = vec![
                "abi".to_string(),
                "evm.bytecode".to_string(),
                "evm.deployedBytecode".to_string(),
                "evm.methodIdentifiers".to_string(),
            ];
            for (_key, value) in self.settings.output_selection.iter_mut() {
                value.clone_from(&default_output_selection);
            }
        } else {
            self.settings.output_selection = OutputSelection::default_file_output_selection()
        }
    }

    fn modified_copy(&self) -> Self {
        let mut copy = self.clone();
        copy.sources.iter_mut().for_each(|(_file, source)| {
            let mut modified_content = source.content.as_ref().clone();
            modified_content.push(' ');
            source.content = Arc::new(modified_content);
        });
        copy
    }

    fn language(&self) -> Language {
        Language::Vyper
    }

    fn settings(&self) -> Value {
        serde_json::to_value(&self.settings).expect("failed to serialize settings")
    }

    fn sources(&self) -> BTreeMap<String, String> {
        let mut sources = BTreeMap::new();
        for (file_path, source) in self.sources.clone() {
            sources.insert(
                file_path.to_string_lossy().to_string(),
                source.content.as_ref().clone(),
            );
        }
        sources
    }
}

#[derive(Debug, Default)]
pub struct VyperCompiler {}

#[async_trait]
impl evm_compilers::EvmCompiler for VyperCompiler {
    type CompilerInput = VyperInput;

    async fn compile(
        compiler_path: &Path,
        compiler_version: &DetailedVersion,
        input: &Self::CompilerInput,
    ) -> Result<Value, Error> {
        let solc = foundry_compilers_new::solc::Solc::new_with_version(
            compiler_path,
            compiler_version.to_semver().to_owned(),
        );
        let output = solc
            .async_compile_output(input)
            .await
            .context("compilation")?;
        let output_value =
            serde_json::from_slice(&output).context("deserializing compiler output into value")?;

        Ok(output_value)
    }
}
