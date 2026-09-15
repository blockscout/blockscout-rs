// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{
    compiler::{
        CommandArgument, CompactVersion, CompilerExecutor, CompilerInvocation,
        ConcurrencyLimitedCompilerExecutor, DetailedVersion, DownloadCache, Fetcher, JobFile,
    },
    zksync::zksolc_standard_json::{input, input::Input, output, output::contract::Contract},
    Version,
};
use anyhow::Context;
use async_trait::async_trait;
use bytes::Bytes;
use foundry_compilers::error::SolcError;
use futures::TryFutureExt;
use nonempty::NonEmpty;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Arc,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::zksync::solidity::verification_success::Language;
use thiserror::Error;
use tokio::sync::Semaphore;
use verification_common::verifier_alliance::{
    CompilationArtifacts, CreationCodeArtifacts, Match, MatchBuilder, RuntimeCodeArtifacts,
    ToCompilationArtifacts, ToCreationCodeArtifacts, ToRuntimeCodeArtifacts,
};

#[derive(Clone, Debug)]
pub struct VerificationRequest {
    pub code: Bytes,
    pub constructor_arguments: Option<Bytes>,
    pub zk_compiler: CompactVersion,
    pub solc_compiler: DetailedVersion,
    pub content: Input,
}

#[derive(Clone, Debug)]
pub struct VerificationSuccess {
    pub file_path: String,
    pub contract_name: String,
    pub compilation_artifacts: CompilationArtifacts,
    pub creation_code_artifacts: CreationCodeArtifacts,
    pub runtime_code_artifacts: RuntimeCodeArtifacts,
    pub creation_match: Option<Match>,
    pub runtime_match: Match,
}

#[derive(Clone, Debug)]
pub struct VerificationFailure {
    pub file_path: String,
    pub contract_name: String,
    pub message: String,
}

pub struct VerificationResult {
    pub zk_compiler: String,
    pub zk_compiler_version: CompactVersion,
    pub evm_compiler: String,
    pub evm_compiler_version: DetailedVersion,
    pub language: Language,
    pub compiler_settings: Value,
    pub sources: BTreeMap<String, String>,
    pub successes: Vec<VerificationSuccess>,
    pub failures: Vec<VerificationFailure>,
}

// TODO: uses on assumption, that only full matches are detected. Should be
//       updated, when zksync verifier starts to detect partial matches as well.
pub fn choose_best_success(successes: Vec<VerificationSuccess>) -> Option<VerificationSuccess> {
    let mut best = None;
    for success in successes {
        if success.creation_match.is_some() {
            best = Some(success);
            break;
        } else if best.is_none() {
            best = Some(success)
        }
    }

    best
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Zk compiler not found: {0}")]
    ZkCompilerNotFound(String),
    #[error("Evm compiler not found: {0}")]
    EvmCompilerNotFound(String),
    #[error("Compilation error: {0:?}")]
    Compilation(Vec<String>),
    #[error("{0:#?}")]
    Internal(#[from] anyhow::Error),
}

pub async fn verify(
    compilers: &ZkSyncCompilers<ZkSolcCompiler>,
    request: VerificationRequest,
) -> Result<VerificationResult, Error> {
    let zk_compiler_version = request.zk_compiler;
    let evm_compiler_version = request.solc_compiler;
    let mut compiler_input = request.content;

    compiler_input.normalize_output_selection(&zk_compiler_version);

    // retrieves both usual solidity and zksync era solidity evm compiler versions
    // matching to the requested evm compiler version.
    // zk_compiler version is checked to exist, so also returned in the response.
    let (zk_compiler, evm_compilers) = compilers
        .extract_compiler_versions_to_fetch(&zk_compiler_version, &evm_compiler_version)?;

    let mut successes = vec![];
    let mut failures = vec![];
    for evm_compiler in evm_compilers {
        let (zk_compiler_path, evm_compiler_path) = compilers
            .fetch_compilers(&zk_compiler, &evm_compiler)
            .await?;

        let (compiler_output, _raw_compiler_output) = compilers
            .compile(&zk_compiler_path, &evm_compiler_path, &compiler_input)
            .await?;

        (successes, failures) = verify_compiler_output(
            request.code.clone(),
            request.constructor_arguments.clone(),
            compiler_output,
        )?;

        if !successes.is_empty() {
            break;
        }
    }

    let sources = compiler_input
        .sources
        .into_iter()
        .map(|(file, source)| (file, source.content.to_string()))
        .collect();
    Ok(VerificationResult {
        zk_compiler: "zksolc".to_string(),
        zk_compiler_version,
        evm_compiler: "solc".to_string(),
        evm_compiler_version,
        language: Language::Solidity,
        compiler_settings: serde_json::to_value(compiler_input.settings)
            .context("compiler settings serialization")?,
        sources,
        successes,
        failures,
    })
}

fn verify_compiler_output(
    code: Bytes,
    constructor_arguments: Option<Bytes>,
    compiler_output: output::Output,
) -> Result<(Vec<VerificationSuccess>, Vec<VerificationFailure>), Error> {
    let mut successes = Vec::new();
    let mut failures = Vec::new();
    for (file, contracts) in compiler_output.contracts.unwrap_or_default() {
        for (name, contract) in contracts {
            match check_contract(
                file.clone(),
                name,
                code.clone(),
                constructor_arguments.clone(),
                contract,
            )? {
                Ok(success) => {
                    let creation_match_type = success.creation_match.as_ref().map(|v| {
                        if v.metadata_match {
                            "full"
                        } else {
                            "partial"
                        }
                    });
                    let runtime_match_type = if success.runtime_match.metadata_match {
                        "full"
                    } else {
                        "partial"
                    };
                    tracing::trace!(
                        file = success.file_path,
                        contract = success.contract_name,
                        "contract matches; creation_match={creation_match_type:?}, runtime_match={runtime_match_type}",
                    );
                    successes.push(success);
                }
                Err(failure) => {
                    tracing::trace!(
                        file = failure.file_path,
                        contract = failure.contract_name,
                        "contract does not match; error={}",
                        failure.message
                    );
                    failures.push(failure);
                }
            }
        }
    }

    Ok((successes, failures))
}

fn check_contract(
    file_path: String,
    contract_name: String,
    code: Bytes,
    constructor_arguments: Option<Bytes>,
    contract: Contract,
) -> Result<Result<VerificationSuccess, VerificationFailure>, anyhow::Error> {
    let failure = |message: String| {
        Ok(Err(VerificationFailure {
            file_path: file_path.clone(),
            contract_name: contract_name.clone(),
            message,
        }))
    };

    if let Some(bytecode) = contract.evm.as_ref().and_then(|evm| evm.bytecode.as_ref()) {
        let compiled_code = blockscout_display_bytes::decode_hex(&bytecode.object);
        if let Ok(compiled_code) = compiled_code {
            let compilation_artifacts = CompilationArtifacts::from(&contract);
            let creation_code_artifacts = CreationCodeArtifacts::from(&contract);
            let runtime_code_artifacts = RuntimeCodeArtifacts::from(&contract);

            let runtime_match = build_runtime_match(
                code.as_ref(),
                compiled_code.clone(),
                &runtime_code_artifacts,
            )
            .context("runtime match")?;
            if let Some(runtime_match) = runtime_match {
                let mut creation_code = code.to_vec();
                creation_code.extend(constructor_arguments.unwrap_or_default());
                let creation_match = build_creation_match(
                    &creation_code,
                    compiled_code,
                    &creation_code_artifacts,
                    &compilation_artifacts,
                )
                .context("creation match")?;
                Ok(Ok(VerificationSuccess {
                    file_path,
                    contract_name,
                    compilation_artifacts,
                    creation_code_artifacts,
                    runtime_code_artifacts,
                    creation_match,
                    runtime_match,
                }))
            } else {
                failure("compiled bytecode does not match the deployed one".into())
            }
        } else {
            failure(format!(
                "compiled bytecode.object is not a valid hex; object={}; err={}",
                bytecode.object,
                compiled_code.unwrap_err()
            ))
        }
    } else {
        failure("compiled bytecode is null".into())
    }
}

fn build_runtime_match(
    code: &[u8],
    compiled_code: Vec<u8>,
    runtime_code_artifacts: &RuntimeCodeArtifacts,
) -> Result<Option<Match>, anyhow::Error> {
    let runtime_match_builder = MatchBuilder::new(code, compiled_code.clone());
    let runtime_match = if let Some(runtime_match_builder) = runtime_match_builder {
        runtime_match_builder
            .set_has_cbor_auxdata(true)
            .apply_runtime_code_transformations(runtime_code_artifacts)
            .context("applying transformations")?
            .verify_and_build()
    } else {
        None
    };
    Ok(runtime_match)
}

fn build_creation_match(
    code: &[u8],
    compiled_code: Vec<u8>,
    creation_code_artifacts: &CreationCodeArtifacts,
    compilation_artifacts: &CompilationArtifacts,
) -> Result<Option<Match>, anyhow::Error> {
    let creation_match_builder = MatchBuilder::new(code, compiled_code);
    let creation_match = if let Some(creation_match_builder) = creation_match_builder {
        creation_match_builder
            .set_has_cbor_auxdata(true)
            .apply_creation_code_transformations(creation_code_artifacts, compilation_artifacts)
            .context("applying transformations")?
            .verify_and_build()
    } else {
        None
    };
    Ok(creation_match)
}

pub trait CompilerInput {
    /// Modifies input so that the corresponding bytecode
    /// should have modified metadata hash, if any.
    fn modify(self) -> Self;

    fn normalize_output_selection(&mut self, version: &CompactVersion);
}

impl CompilerInput for Input {
    fn modify(mut self) -> Self {
        // TODO: could we update some other field to avoid copying strings?
        self.sources.iter_mut().for_each(|(_file, source)| {
            let mut modified_content = source.content.as_ref().clone();
            modified_content.push(' ');
            source.content = Arc::new(modified_content);
        });
        self
    }

    fn normalize_output_selection(&mut self, version: &CompactVersion) {
        use input::settings::selection::{
            file::{flag::Flag, File},
            Selection,
        };
        let per_contract_flags = if version.to_semver() < &semver::Version::new(1, 3, 6) {
            HashSet::from([Flag::ABI, Flag::StorageLayout])
        } else {
            HashSet::from([Flag::ABI, Flag::Devdoc, Flag::Userdoc, Flag::StorageLayout])
        };
        let output_selection = Selection {
            all: Some(File {
                per_file: None,
                per_contract: Some(per_contract_flags),
            }),
        };
        self.settings.output_selection = Some(output_selection);
    }
}

pub trait CompilerOutput {
    fn check_errors(&self) -> Option<NonEmpty<String>>;
}

impl CompilerOutput for output::Output {
    fn check_errors(&self) -> Option<NonEmpty<String>> {
        // Compilations errors, warnings and info messages are returned in `CompilerOutput.errors`
        let mut errors = Vec::new();
        for err in self.errors.clone().unwrap_or_default() {
            if err.severity == "error" {
                errors.push(err.formatted_message.clone())
            }
        }
        NonEmpty::from_vec(errors)
    }
}

impl ToCompilationArtifacts for Contract {
    fn abi(&self) -> Option<Value> {
        self.abi.clone()
    }

    fn devdoc(&self) -> Option<Value> {
        self.devdoc.clone()
    }

    fn userdoc(&self) -> Option<Value> {
        self.userdoc.clone()
    }
    fn storage_layout(&self) -> Option<Value> {
        self.storage_layout.clone()
    }
}

impl ToCreationCodeArtifacts for Contract {}

impl ToRuntimeCodeArtifacts for Contract {}

#[async_trait]
pub trait ZkSyncCompiler {
    type CompilerInput;
    type CompilerOutput: CompilerOutput + DeserializeOwned;

    async fn compile(
        executor: &dyn CompilerExecutor,
        zk_compiler_path: &Path,
        evm_compiler_path: &Path,
        input: &Self::CompilerInput,
    ) -> Result<Value, SolcError>;
}

pub struct ZkSyncCompilers<ZkC> {
    evm_cache: DownloadCache<DetailedVersion>,
    evm_fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
    era_evm_cache: DownloadCache<DetailedVersion>,
    era_evm_fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
    zk_cache: DownloadCache<CompactVersion>,
    zk_fetcher: Arc<dyn Fetcher<Version = CompactVersion>>,
    executor: Arc<dyn CompilerExecutor>,
    _phantom_data: PhantomData<ZkC>,
}

impl<ZkC: ZkSyncCompiler> ZkSyncCompilers<ZkC> {
    pub fn new_with_executor(
        evm_fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
        era_evm_fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
        zk_fetcher: Arc<dyn Fetcher<Version = CompactVersion>>,
        threads_semaphore: Arc<Semaphore>,
        executor: Arc<dyn CompilerExecutor>,
    ) -> Self {
        let executor = Arc::new(ConcurrencyLimitedCompilerExecutor::with_semaphore(
            executor,
            threads_semaphore,
        ));
        Self::new_with_admitted_executor(evm_fetcher, era_evm_fetcher, zk_fetcher, executor)
    }

    pub fn new_with_admitted_executor(
        evm_fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
        era_evm_fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
        zk_fetcher: Arc<dyn Fetcher<Version = CompactVersion>>,
        executor: Arc<dyn CompilerExecutor>,
    ) -> Self {
        Self {
            evm_cache: DownloadCache::default(),
            evm_fetcher,
            era_evm_cache: DownloadCache::default(),
            era_evm_fetcher,
            zk_cache: DownloadCache::default(),
            zk_fetcher,
            executor,
            _phantom_data: Default::default(),
        }
    }

    pub async fn compile(
        &self,
        zk_compiler_path: &Path,
        evm_compiler_path: &Path,
        input: &ZkC::CompilerInput,
    ) -> Result<(ZkC::CompilerOutput, Value), Error> {
        let raw_compiler_output = ZkC::compile(
            self.executor.as_ref(),
            zk_compiler_path,
            evm_compiler_path,
            input,
        )
        .await
        .context("compilation")?;

        let compiler_output = ZkC::CompilerOutput::deserialize(&raw_compiler_output)
            .context("deserializing compiler output")?;
        if let Some(errors) = compiler_output.check_errors() {
            return Err(Error::Compilation(errors.into()));
        }

        Ok((compiler_output, raw_compiler_output))
    }

    pub fn all_evm_versions_sorted_str(&self) -> Vec<String> {
        let mut versions = self.evm_fetcher.all_versions();
        // sort in descending order
        versions.sort_by(|x, y| x.cmp(y).reverse());
        versions.into_iter().map(|v| v.to_string()).collect()
    }

    pub fn all_zk_versions_sorted_str(&self) -> Vec<String> {
        let mut versions = self.zk_fetcher.all_versions();
        // sort in descending order
        versions.sort_by(|x, y| x.cmp(y).reverse());
        versions.into_iter().map(|v| v.to_string()).collect()
    }
}

pub struct FetchableEvmCompiler {
    version: DetailedVersion,
    is_era_evm_compiler: bool,
}

impl<ZkC: ZkSyncCompiler> ZkSyncCompilers<ZkC> {
    pub async fn fetch_compilers(
        &self,
        zk_compiler: &CompactVersion,
        evm_compiler: &FetchableEvmCompiler,
    ) -> Result<(PathBuf, PathBuf), Error> {
        let zk_path_future = self
            .zk_cache
            .get(self.zk_fetcher.as_ref(), zk_compiler)
            .map_err(|err| {
                Error::Internal(anyhow::Error::new(err).context("fetching zk compiler"))
            });

        let (cache, fetcher) = if evm_compiler.is_era_evm_compiler {
            (&self.era_evm_cache, self.era_evm_fetcher.as_ref())
        } else {
            (&self.evm_cache, self.evm_fetcher.as_ref())
        };

        let evm_path_future = cache.get(fetcher, &evm_compiler.version).map_err(|err| {
            Error::Internal(anyhow::Error::new(err).context("fetching zk compiler"))
        });

        let (zk_path_result, evm_path_result) = futures::join!(zk_path_future, evm_path_future);

        Ok((zk_path_result?, evm_path_result?))
    }

    fn extract_compiler_versions_to_fetch(
        &self,
        zk_compiler: &CompactVersion,
        evm_compiler: &DetailedVersion,
    ) -> Result<(CompactVersion, NonEmpty<FetchableEvmCompiler>), Error> {
        let all_zk_compilers = self.zk_fetcher.all_versions();
        let zk_compiler = match all_zk_compilers
            .into_iter()
            .find(|value| zk_compiler == value)
        {
            None => return Err(Error::ZkCompilerNotFound(zk_compiler.to_string())),
            Some(zk_compiler) => zk_compiler,
        };

        let all_evm_compilers = self.evm_fetcher.all_versions();
        let maybe_requested_evm_compiler = all_evm_compilers
            .contains(evm_compiler)
            .then(|| evm_compiler.clone());

        let matching_era_evm_compilers =
            self.lookup_era_evm_compilers_by_semver_version(evm_compiler.version());

        let mut fetchable_evm_compilers: Vec<_> = matching_era_evm_compilers
            .into_iter()
            .map(|version| FetchableEvmCompiler {
                version,
                is_era_evm_compiler: true,
            })
            .collect();

        if let Some(requested_evm_compiler) = maybe_requested_evm_compiler {
            fetchable_evm_compilers.push(FetchableEvmCompiler {
                version: requested_evm_compiler,
                is_era_evm_compiler: false,
            })
        }

        match NonEmpty::from_vec(fetchable_evm_compilers) {
            None => Err(Error::EvmCompilerNotFound(evm_compiler.to_string())),
            Some(fetchable_evm_compilers) => Ok((zk_compiler, fetchable_evm_compilers)),
        }
    }

    fn lookup_era_evm_compilers_by_semver_version(
        &self,
        version: &semver::Version,
    ) -> Vec<DetailedVersion> {
        let era_evm_compilers = self.era_evm_fetcher.all_versions();
        era_evm_compilers
            .into_iter()
            .filter(|compiler| compiler.version() == version)
            .collect()
    }
}

#[derive(Default)]
pub struct ZkSolcCompiler {}

#[async_trait]
impl ZkSyncCompiler for ZkSolcCompiler {
    type CompilerInput = Input;
    type CompilerOutput = output::Output;

    async fn compile(
        executor: &dyn CompilerExecutor,
        zk_compiler_path: &Path,
        evm_compiler_path: &Path,
        input: &Self::CompilerInput,
    ) -> Result<Value, SolcError> {
        let input = serde_json::to_vec(input)?;
        let invocation = CompilerInvocation::new(
            JobFile::executable("zksolc", "bin/zksolc", zk_compiler_path)
                .map_err(|err| SolcError::Message(err.to_string()))?,
            vec![
                CommandArgument::prefixed_file("--solc=", "solc"),
                CommandArgument::literal("--standard-json"),
            ],
            input,
        )
        .with_file(
            JobFile::executable("solc", "bin/solc", evm_compiler_path)
                .map_err(|err| SolcError::Message(err.to_string()))?,
        );
        let output = executor
            .execute(invocation)
            .await
            .map_err(|err| SolcError::Message(err.to_string()))?;
        output
            .ensure_success("zksolc")
            .map_err(|err| SolcError::Message(err.to_string()))?;
        serde_json::from_slice(&output.stdout).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExecutionError, ExecutionOutput};
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingExecutor {
        invocation: Mutex<Option<CompilerInvocation>>,
    }

    #[async_trait]
    impl CompilerExecutor for RecordingExecutor {
        async fn execute(
            &self,
            invocation: CompilerInvocation,
        ) -> Result<ExecutionOutput, ExecutionError> {
            invocation.validate()?;
            *self.invocation.lock().unwrap() = Some(invocation);
            Ok(ExecutionOutput {
                stdout: Bytes::from_static(b"{}"),
                stderr: Bytes::new(),
                exit_code: 0,
                oom_killed: false,
            })
        }
    }

    #[tokio::test]
    async fn zksolc_stages_both_compilers_and_keeps_llvm_options_in_stdin() {
        let input: Input = serde_json::from_value(serde_json::json!({
            "language": "Solidity",
            "sources": {},
            "settings": {
                "optimizer": { "enabled": false },
                "LLVMOptions": ["--exec-on-ir-change=/bin/true"]
            }
        }))
        .unwrap();
        let executor = RecordingExecutor::default();

        ZkSolcCompiler::compile(
            &executor,
            Path::new("/cache/zksolc"),
            Path::new("/cache/solc"),
            &input,
        )
        .await
        .unwrap();

        let invocation = executor.invocation.lock().unwrap().take().unwrap();
        assert_eq!(
            invocation.program_path(Path::new("/job")).unwrap(),
            Path::new("/job/bin/zksolc")
        );
        assert_eq!(
            invocation.resolved_args(Path::new("/job")).unwrap(),
            ["--solc=/job/bin/solc", "--standard-json"]
        );
        assert_eq!(invocation.files().len(), 2);
        assert!(String::from_utf8_lossy(invocation.stdin()).contains("exec-on-ir-change"));
    }
}
