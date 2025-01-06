use super::artifacts::CompilerInput;
use crate::compiler::{self, DetailedVersion, EvmCompiler};
use ethers_solc::{error::SolcError, CompilerOutput, Solc};
use foundry_compilers::artifacts::output_selection::OutputSelection;
use std::path::Path;

#[derive(Default)]
pub struct VyperCompiler {}

impl VyperCompiler {
    pub fn new() -> Self {
        VyperCompiler {}
    }
}

impl compiler::CompilerInput for CompilerInput {
    fn modify(mut self) -> Self {
        self.sources.iter_mut().for_each(|(_file, source)| {
            let mut modified_content = source.content.as_ref().clone();
            modified_content.push(' ');
            source.content = std::sync::Arc::new(modified_content);
        });
        self
    }

    // Starting from pre-release versions of 0.4.0 interfaces are missing from input standard-json.
    // Due to that, we cannot specify output selection for all files (via "*" wildcard),
    // as some of them may be interfaces, which should not be compiled.
    // Thus, we start specifying required outputs only for those files
    // that already exists in the provided output_selection.
    fn normalize_output_selection(&mut self, version: &DetailedVersion) {
        // v0.3.10 was the latest release prior to v0.4.0 pre-releases
        if version.version() > &semver::Version::new(0, 3, 10) {
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
}

#[async_trait::async_trait]
impl EvmCompiler for VyperCompiler {
    type CompilerInput = CompilerInput;

    async fn compile(
        &self,
        path: &Path,
        _ver: &DetailedVersion,
        input: &Self::CompilerInput,
    ) -> Result<(serde_json::Value, CompilerOutput), SolcError> {
        let raw = Solc::from(path).async_compile_output(input).await?;

        let mut raw_output = serde_json::from_slice(&raw)?;
        update_source_map(&mut raw_output);

        let vyper_output: types::VyperCompilerOutput = serde_json::from_value(raw_output.clone())?;

        Ok((raw_output, CompilerOutput::from(vyper_output)))
    }
}

// TODO: that should be replaced with an actual vyper CompilerOutput struct deserialization
fn update_source_map(raw_output: &mut serde_json::Value) {
    let update = |bytecode: &mut serde_json::Value| {
        if let Some(bytecode) = bytecode.as_object_mut() {
            if let Some(source_map) = bytecode.get("sourceMap") {
                if let Some(source_map_object) = source_map.as_object() {
                    if let Some(source_map_string) = source_map_object.get("pc_pos_map_compressed")
                    {
                        bytecode.insert("sourceMap".to_string(), source_map_string.clone());
                    }
                }
            }
        }
    };

    if let Some(output) = raw_output.as_object_mut() {
        if let Some(contract_files) = output.get_mut("contracts") {
            if let Some(contract_files) = contract_files.as_object_mut() {
                for (_file, contracts) in contract_files {
                    if let Some(contracts) = contracts.as_object_mut() {
                        for (_name, contract) in contracts {
                            if let Some(bytecode) = contract.pointer_mut("/evm/bytecode") {
                                update(bytecode)
                            }
                            if let Some(deployed_bytecode) =
                                contract.pointer_mut("/evm/deployedBytecode")
                            {
                                update(deployed_bytecode)
                            }
                        }
                    }
                }
            }
        }
    }
}

mod types {
    use std::collections::BTreeMap;

    use ethers_solc::{
        artifacts::{Contract, Error, Severity, SourceFile},
        CompilerOutput,
    };
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
    pub struct SourceLocation {
        pub file: String,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct VyperError {
        pub r#type: String,
        pub component: String,
        pub severity: Severity,
        pub message: String,
        pub formatted_message: Option<String>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
    pub struct VyperCompilerOutput {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub errors: Vec<VyperError>,
        #[serde(default)]
        pub contracts: BTreeMap<String, BTreeMap<String, Contract>>,
        #[serde(default)]
        pub sources: BTreeMap<String, SourceFile>,
    }

    impl From<VyperCompilerOutput> for CompilerOutput {
        fn from(vyper: VyperCompilerOutput) -> Self {
            let errors = vyper
                .errors
                .into_iter()
                .map(|e| Error {
                    r#type: e.r#type,
                    component: e.component,
                    severity: e.severity,
                    message: e.message,
                    formatted_message: e.formatted_message,
                    source_location: None,
                    secondary_source_locations: vec![],
                    error_code: None,
                })
                .collect();
            CompilerOutput {
                errors,
                sources: vyper.sources,
                contracts: vyper.contracts,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compiler::{self, Compilers, ListFetcher},
        consts::DEFAULT_VYPER_COMPILER_LIST,
    };
    use foundry_compilers::artifacts::Source;
    use std::{
        collections::{BTreeMap, HashSet},
        env::temp_dir,
        path::PathBuf,
        str::FromStr,
        sync::Arc,
    };
    use tokio::sync::{OnceCell, Semaphore};

    async fn global_compilers() -> &'static Compilers<VyperCompiler> {
        static COMPILERS: OnceCell<Compilers<VyperCompiler>> = OnceCell::const_new();
        COMPILERS
            .get_or_init(|| async {
                let url = DEFAULT_VYPER_COMPILER_LIST.try_into().expect("Getting url");
                let fetcher = ListFetcher::new(url, temp_dir(), None, None)
                    .await
                    .expect("Fetch releases");
                let threads_semaphore = Arc::new(Semaphore::new(4));
                Compilers::new(Arc::new(fetcher), VyperCompiler::new(), threads_semaphore)
            })
            .await
    }

    fn input_with_sources(sources: BTreeMap<PathBuf, String>) -> CompilerInput {
        let mut compiler_input = CompilerInput {
            language: "Vyper".to_string(),
            sources: sources
                .into_iter()
                .map(|(name, content)| (name, Source::new(content)))
                .collect(),
            interfaces: Default::default(),
            settings: Default::default(),
        };
        compiler_input.settings.evm_version = None;
        compiler_input
    }

    fn input_with_source(source_code: String) -> CompilerInput {
        input_with_sources(BTreeMap::from([("source.vy".into(), source_code)]))
    }

    #[tokio::test]
    async fn compile_success() {
        let source_code = r#"
# @version ^0.3.1

userName: public(String[100])

@external
def __init__(name: String[100]):
    self.userName = name

@view
@external
def getUserName() -> String[100]:
    return self.userName
"#;

        let compilers = global_compilers().await;
        let input: CompilerInput = input_with_source(source_code.into());
        let version =
            compiler::DetailedVersion::from_str("0.3.6+commit.4a2124d0").expect("Compiler version");

        let (_raw, result) = compilers
            .compile(&version, &input, None)
            .await
            .expect("Compilation failed");
        let contracts: HashSet<String> =
            result.contracts_into_iter().map(|(name, _)| name).collect();
        assert_eq!(
            contracts,
            HashSet::from_iter(["source".into()]),
            "compilation output should contain 1 contract",
        )
    }

    #[tokio::test]
    async fn compile_failed() {
        let compilers = global_compilers().await;
        let version = compiler::DetailedVersion::from_str("v0.2.11+commit.5db35ef")
            .expect("Compiler version");

        for sources in [
            BTreeMap::from_iter([("source.vy".into(), "some wrong vyper code".into())]),
            BTreeMap::from_iter([(
                "source.vy".into(),
                "\n\n# @version =0.3.1\n\n# wrong vyper version".into(),
            )]),
        ] {
            let input = input_with_sources(sources);
            let _ = compilers
                .compile(&version, &input, None)
                .await
                .expect_err("Compilation should fail");
        }
    }
}
