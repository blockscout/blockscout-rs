use super::Error;
use crate::{
    compiler::DownloadCache, verify_new::compiler_output::SharedCompilerOutput, DetailedVersion,
    Fetcher, Language,
};
use anyhow::Context;
use async_trait::async_trait;
use foundry_compilers_new::CompilationError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Semaphore;

#[async_trait]
pub trait EvmCompiler {
    type CompilerInput: CompilerInput;
    type CompilationError: CompilationError + for<'de> Deserialize<'de>;
    // TODO: parameterize version via: `type Version: Version`

    async fn compile(
        compiler_path: &Path,
        compiler_version: &DetailedVersion,
        input: &Self::CompilerInput,
    ) -> Result<Value, Error>;
}

pub trait CompilerInput: Serialize {
    fn normalize_output_selection(&mut self, version: &semver::Version);

    /// Modifies input so that the corresponding bytecode
    /// should have modified metadata hash, if any.
    fn modified_copy(&self) -> Self;

    fn language(&self) -> Language;

    fn settings(&self) -> Value;

    fn sources(&self) -> BTreeMap<String, String>;
}

pub struct CompileResult<CompilerOutput> {
    pub output: CompilerOutput,
    pub raw: Value,
}

pub struct EvmCompilersPool<C: EvmCompiler> {
    cache: DownloadCache<DetailedVersion>,
    fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
    threads_semaphore: Arc<Semaphore>,
    _phantom_data: PhantomData<C>,
}

impl<C: EvmCompiler> EvmCompilersPool<C> {
    pub fn new(
        fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
        threads_semaphore: Arc<Semaphore>,
    ) -> Self {
        Self {
            cache: Default::default(),
            fetcher,
            threads_semaphore,
            _phantom_data: Default::default(),
        }
    }

    pub fn normalize_compiler_version(
        &self,
        to_normalize: &DetailedVersion,
    ) -> Result<DetailedVersion, Error> {
        let is_matching_version = |version: &&DetailedVersion| {
            version.version() == to_normalize.version()
                && version.date() == to_normalize.date()
                && (version.commit().starts_with(to_normalize.commit())
                    || to_normalize.commit().starts_with(version.commit()))
        };

        let all_versions = self.fetcher.all_versions();
        let normalized = all_versions.iter().find(is_matching_version);

        if let Some(normalized) = normalized {
            Ok(normalized.clone())
        } else {
            Err(Error::CompilerNotFound(to_normalize.to_string()))
        }
    }

    pub async fn fetch_compiler(&self, version: &DetailedVersion) -> Result<PathBuf, Error> {
        let path = self
            .cache
            .get(self.fetcher.as_ref(), version)
            .await
            .map_err(|err| {
                Error::Internal(anyhow::Error::new(err).context("fetching evm compiler"))
            })?;
        Ok(path)
    }

    pub async fn compile(
        &self,
        compiler_path: &Path,
        compiler_version: &DetailedVersion,
        input: &C::CompilerInput,
    ) -> Result<CompileResult<SharedCompilerOutput>, Error> {
        let _permit = self
            .threads_semaphore
            .acquire()
            .await
            .context("acquiring lock")?;

        let raw = C::compile(compiler_path, compiler_version, input).await?;

        validate_no_errors::<C::CompilationError>(&raw)?;
        let output: SharedCompilerOutput =
            serde_path_to_error::deserialize(&raw).context("deserializing compiler output")?;

        Ok(CompileResult { output, raw })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CompilerOutputErrors<E> {
    #[serde(default = "Vec::new")]
    pub errors: Vec<E>,
}

fn validate_no_errors<E>(raw_output: &Value) -> Result<(), Error>
where
    E: CompilationError + for<'de> Deserialize<'de>,
{
    let output_errors: CompilerOutputErrors<E> = serde_path_to_error::deserialize(raw_output)
        .context("deserializing compiler output errors")?;

    let mut errors = Vec::new();
    for error in output_errors.errors {
        if error.is_error() {
            errors.push(error.to_string())
        }
    }
    if !errors.is_empty() {
        return Err(Error::Compilation(errors));
    }

    Ok(())
}
