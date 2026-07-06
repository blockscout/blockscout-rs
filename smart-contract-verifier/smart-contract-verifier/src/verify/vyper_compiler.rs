// SPDX-License-Identifier: LicenseRef-Blockscout

use super::{evm_compilers, Error};
use crate::{DetailedVersion, Language, Version};
use anyhow::Context;
use async_trait::async_trait;
use foundry_compilers::artifacts;
use serde_json::Value;
use std::{collections::BTreeMap, path::Path, sync::Arc};

pub use super::vyper_compiler_input::VyperInput;

impl evm_compilers::CompilerInput for VyperInput {
    // Starting from pre-release versions of 0.4.0 interfaces are missing from input standard-json.
    // Due to that, we cannot specify output selection for all files (via "*" wildcard),
    // as some of them may be interfaces, which should not be compiled.
    // Thus, we start specifying required outputs only for those files
    // that already exists in the provided output_selection.
    fn normalize_output_selection(&mut self, version: &semver::Version) {
        let default_output_selection = vec![
            "abi".to_string(),
            "evm.bytecode".to_string(),
            "evm.deployedBytecode".to_string(),
            "evm.methodIdentifiers".to_string(),
        ];
        // If the request omitted `outputSelection`, populate it with the top-level contract
        // sources so that the compiler actually emits contracts. We exclude files that were
        // provided only to satisfy imports from library search paths (e.g. snekmate embedded
        // under `.venv/.../site-packages`): selecting such library modules directly makes vyper
        // fail with "module is used but not initialized". Files under the `"."` search path (or
        // when no library search paths are set) are treated as top-level contracts.
        if self.settings.output_selection.is_empty() {
            let library_prefixes: Vec<&str> = self
                .settings
                .search_paths
                .iter()
                .map(String::as_str)
                .filter(|p| *p != ".")
                .collect();
            for path in self.sources.keys() {
                let path = path.to_string_lossy();
                let is_library = library_prefixes
                    .iter()
                    .any(|prefix| path.starts_with(prefix));
                if !is_library {
                    self.settings
                        .output_selection
                        .insert(path.into_owned(), default_output_selection.clone());
                }
            }
        }
        // v0.3.10 was the latest release prior to v0.4.0 pre-releases
        if version > &semver::Version::new(0, 3, 10) {
            for (_key, value) in self.settings.output_selection.iter_mut() {
                value.clone_from(&default_output_selection);
            }
        } else {
            self.settings.output_selection =
                BTreeMap::from([("*".to_string(), default_output_selection)]);
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
        for (file_path, interface) in self.interfaces.clone() {
            sources.insert(file_path.to_string_lossy().to_string(), interface.content());
        }
        sources
    }
}

impl evm_compilers::CompilationError for artifacts::vyper::VyperCompilationError {
    fn formatted_message(&self) -> String {
        self.formatted_message
            .clone()
            .unwrap_or(self.message.clone())
    }
}

#[derive(Debug, Default)]
pub struct VyperCompiler {}

#[async_trait]
impl evm_compilers::EvmCompiler for VyperCompiler {
    type CompilerInput = VyperInput;
    type CompilationError = artifacts::vyper::VyperCompilationError;

    async fn compile(
        compiler_path: &Path,
        compiler_version: &DetailedVersion,
        input: &Self::CompilerInput,
    ) -> Result<Value, Error> {
        // we use `solc::Solc` because `solc::Solc` does the same thing under the hood.
        let solc = foundry_compilers::solc::Solc::new_with_version(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::{evm_compilers::CompilerInput, vyper_compiler_input};
    use foundry_compilers::artifacts::{Source, Sources};
    use std::path::PathBuf;

    fn build_input(sources: &[&str], search_paths: &[&str]) -> VyperInput {
        let sources: Sources = sources
            .iter()
            .map(|path| (PathBuf::from(path), Source::new("")))
            .collect();
        VyperInput {
            language: "Vyper".to_string(),
            sources,
            interfaces: Default::default(),
            settings: vyper_compiler_input::Settings {
                evm_version: None,
                optimize: None,
                bytecode_metadata: None,
                // Empty, mirroring a standard-json request that omits `outputSelection`
                // (serde default for the field), as opposed to `Settings::default()` which
                // pre-populates a `{"*": ..}` selection.
                output_selection: Default::default(),
                search_paths: search_paths.iter().map(|s| s.to_string()).collect(),
            },
        }
    }

    #[test]
    fn omitted_output_selection_excludes_library_search_paths() {
        let mut input = build_input(
            &[
                "contracts/dao/LiquidityGauge.vy",
                "contracts/dao/erc4626.vy",
                ".venv/lib/pypy3.11/site-packages/snekmate/utils/math.vy",
            ],
            &[".venv/lib/pypy3.11/site-packages", "."],
        );
        input.normalize_output_selection(&semver::Version::new(0, 4, 3));

        let selected: Vec<_> = input.settings.output_selection.keys().cloned().collect();
        assert_eq!(
            selected,
            vec![
                "contracts/dao/LiquidityGauge.vy".to_string(),
                "contracts/dao/erc4626.vy".to_string(),
            ],
            "only top-level contracts should be selected, not sources under library search paths"
        );
    }

    #[test]
    fn omitted_output_selection_without_library_paths_selects_all_sources() {
        let mut input = build_input(&["a.vy", "b.vy"], &["."]);
        input.normalize_output_selection(&semver::Version::new(0, 4, 3));

        let selected: Vec<_> = input.settings.output_selection.keys().cloned().collect();
        assert_eq!(selected, vec!["a.vy".to_string(), "b.vy".to_string()]);
    }

    #[test]
    fn existing_output_selection_is_preserved() {
        let mut input = build_input(
            &["a.vy", "libs/dep.vy"],
            &["libs", "."],
        );
        input
            .settings
            .output_selection
            .insert("a.vy".to_string(), vec!["evm.bytecode".to_string()]);
        input.normalize_output_selection(&semver::Version::new(0, 4, 3));

        // The library-exclusion default only kicks in for an empty output selection, so the
        // provided key set is kept as-is (values are normalized to the defaults).
        let selected: Vec<_> = input.settings.output_selection.keys().cloned().collect();
        assert_eq!(selected, vec!["a.vy".to_string()]);
    }
}
