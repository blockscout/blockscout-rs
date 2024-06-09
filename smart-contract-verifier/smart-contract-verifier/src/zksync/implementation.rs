use crate::{
    compiler::{CompactVersion, DetailedVersion, DownloadCache, FetchError, Fetcher},
    decode_hex,
    zksync::zksolc_standard_json::{input::Input, output, output::contract::Contract},
};
use alloy_dyn_abi::JsonAbiExt;
use anyhow::Context;
use async_trait::async_trait;
use bytes::Bytes;
use foundry_compilers::error::SolcError;
use futures::TryFutureExt;
use nonempty::NonEmpty;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
use tokio::sync::Semaphore;

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
    pub creation_match: bool,
    pub runtime_match: bool,
}

#[derive(Clone, Debug)]
pub struct VerificationFailure {
    pub file_path: String,
    pub contract_name: String,
    pub message: String,
}

pub struct VerificationResult {
    pub successes: Vec<VerificationSuccess>,
    pub failures: Vec<VerificationFailure>,
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

    let (compiler_output, raw_compiler_output) = compilers
        .compile(&zk_compiler_version, &evm_compiler_version, &compiler_input)
        .await?;

    let mut successes = Vec::new();
    let mut failures = Vec::new();
    for (file, contracts) in compiler_output.contracts.unwrap_or_default() {
        for (name, contract) in contracts {
            match check_contract(
                file.clone(),
                name,
                &request.code,
                request.constructor_arguments.as_ref(),
                contract,
            )? {
                Ok(success) => {
                    tracing::trace!(
                        file = success.file_path,
                        contract = success.contract_name,
                        "contract matches; creation_match={}, runtime_match={}",
                        success.creation_match,
                        success.runtime_match
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

    println!("{raw_compiler_output:#?}");

    Ok(VerificationResult {
        successes,
        failures,
    })
}

fn check_contract(
    file_path: String,
    contract_name: String,
    code: &Bytes,
    constructor_arguments: Option<&Bytes>,
    contract: Contract,
) -> Result<Result<VerificationSuccess, VerificationFailure>, anyhow::Error> {
    let failure = |message: String| {
        Ok(Err(VerificationFailure {
            file_path: file_path.clone(),
            contract_name: contract_name.clone(),
            message,
        }))
    };

    if let Some(bytecode) = contract.evm.and_then(|evm| evm.bytecode) {
        let compiled_code = decode_hex(&bytecode.object);
        if let Ok(compiled_code) = compiled_code {
            if compiled_code == code.as_ref() {
                let runtime_match = true;
                let creation_match =
                    check_constructor_arguments(constructor_arguments, contract.abi.as_ref())?;
                Ok(Ok(VerificationSuccess {
                    file_path,
                    contract_name,
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

fn check_constructor_arguments(
    constructor_arguments: Option<&Bytes>,
    raw_abi: Option<&Value>,
) -> Result<bool, anyhow::Error> {
    let are_valid = match (constructor_arguments, raw_abi) {
        (Some(constructor_arguments), Some(abi)) => {
            let abi = alloy_json_abi::JsonAbi::deserialize(abi)
                .context("parsing compiled contract abi")?;
            match abi.constructor {
                None if constructor_arguments.is_empty() => true,
                Some(constructor)
                    if constructor
                        .abi_decode_input(constructor_arguments, true)
                        .is_ok() =>
                {
                    true
                }
                _ => false,
            }
        }
        _ => false,
    };
    Ok(are_valid)
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

    fn normalize_output_selection(&mut self, _version: &CompactVersion) {}
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

#[async_trait]
pub trait ZkSyncCompiler {
    type CompilerInput;
    type CompilerOutput: CompilerOutput + DeserializeOwned;

    async fn compile(
        zk_compiler_path: &Path,
        evm_compiler_path: &Path,
        input: &Self::CompilerInput,
    ) -> Result<Value, SolcError>;
}

pub struct ZkSyncCompilers<ZkC> {
    evm_cache: DownloadCache<DetailedVersion>,
    evm_fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
    zk_cache: DownloadCache<CompactVersion>,
    zk_fetcher: Arc<dyn Fetcher<Version = CompactVersion>>,
    threads_semaphore: Arc<Semaphore>,
    _phantom_data: PhantomData<ZkC>,
}

impl<ZkC: ZkSyncCompiler> ZkSyncCompilers<ZkC> {
    pub fn new(
        evm_fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
        zk_fetcher: Arc<dyn Fetcher<Version = CompactVersion>>,
        threads_semaphore: Arc<Semaphore>,
    ) -> Self {
        Self {
            evm_cache: DownloadCache::default(),
            evm_fetcher,
            zk_cache: DownloadCache::default(),
            zk_fetcher,
            threads_semaphore,
            _phantom_data: Default::default(),
        }
    }

    pub async fn compile(
        &self,
        zk_compiler: &CompactVersion,
        evm_compiler: &DetailedVersion,
        input: &ZkC::CompilerInput,
    ) -> Result<(ZkC::CompilerOutput, Value), Error> {
        let (zk_path, evm_path) = self.fetch_compilers(zk_compiler, evm_compiler).await?;

        let _permit = self
            .threads_semaphore
            .acquire()
            .await
            .context("acquiring lock")?;

        let raw_compiler_output = ZkC::compile(&zk_path, &evm_path, input)
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

impl<ZkC: ZkSyncCompiler> ZkSyncCompilers<ZkC> {
    pub async fn fetch_compilers(
        &self,
        zk_compiler: &CompactVersion,
        evm_compiler: &DetailedVersion,
    ) -> Result<(PathBuf, PathBuf), Error> {
        let zk_path_future = self
            .zk_cache
            .get(self.zk_fetcher.as_ref(), zk_compiler)
            .map_err(|err| match err {
                FetchError::NotFound(version) => Error::ZkCompilerNotFound(version),
                err => anyhow::Error::new(err)
                    .context("fetching zk compiler")
                    .into(),
            });

        let evm_path_future = self
            .evm_cache
            .get(self.evm_fetcher.as_ref(), evm_compiler)
            .map_err(|err| match err {
                FetchError::NotFound(version) => Error::EvmCompilerNotFound(version),
                err => anyhow::Error::new(err)
                    .context("fetching evm compiler")
                    .into(),
            });

        let (zk_path_result, evm_path_result) = futures::join!(zk_path_future, evm_path_future);
        Ok((zk_path_result?, evm_path_result?))
    }
}

#[derive(Default)]
pub struct ZkSolcCompiler {}

#[async_trait]
impl ZkSyncCompiler for ZkSolcCompiler {
    type CompilerInput = Input;
    type CompilerOutput = output::Output;

    async fn compile(
        zk_compiler_path: &Path,
        evm_compiler_path: &Path,
        input: &Self::CompilerInput,
    ) -> Result<Value, SolcError> {
        foundry_compilers::Solc::new(zk_compiler_path)
            .arg(format!("--solc={}", evm_compiler_path.to_string_lossy()))
            .async_compile_as(input)
            .await
    }
}
