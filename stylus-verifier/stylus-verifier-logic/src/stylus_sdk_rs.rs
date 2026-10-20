// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{
    deployment,
    job::{self, clone_exit, JobSettings},
};
use alloy_json_abi::JsonAbi;
use anyhow::{anyhow, Context};
use bytes::Bytes;
use compiler_runner::{
    DockerCompilerExecutor, ExecutionError, SandboxFile, SandboxOutput, SandboxSession,
};
use semver::Version;
use serde::Deserialize;
use serde_with::{json::JsonString, serde_as, PickFirst};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    num::NonZeroUsize,
    path::{Component, Path, PathBuf},
    str::Lines,
    time::Duration,
};
use thiserror::Error;
use tokio::sync::Semaphore;
use url::Url;

/// Name of the toolchain file used to specify the Rust toolchain version for a project.
pub const TOOLCHAIN_FILE_NAME: &str = "rust-toolchain.toml";

/// Name of the file used to specify the package name for a project.
pub const PACKAGE_FILE_NAME: &str = "Cargo.toml";

/// Heads every failed verification.
pub const VERIFICATION_FAILED_MESSAGE: &str =
    "contract deployment did not verify against local project's file hashes";

/// How much of a failed build's output goes into the response.
const MAX_FAILURE_DETAILS_BYTES: usize = 8 * 1024;

pub struct VerifyGithubRepositoryRequest {
    pub deployment_transaction: Bytes,
    pub rpc_endpoint: String,
    pub cargo_stylus_version: Version,
    pub repository_url: Url,
    pub commit: String,
    pub path_prefix: PathBuf,
}

pub struct Success {
    pub abi: Option<serde_json::Value>,
    pub contract_name: Option<String>,
    pub files: BTreeMap<String, String>,
    pub package_name: String,
    pub cargo_stylus_version: Version,
    pub repository_url: Url,
    pub commit: String,
    pub path_prefix: PathBuf,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("{VERIFICATION_FAILED_MESSAGE}\n{0}")]
    VerificationFailed(String),
    #[error("url is not a github repository: {0}")]
    RepositoryIsNotGithub(String),
    #[error("repository not found: {0}")]
    RepositoryNotFound(String),
    #[error("commit hash not found: {0}")]
    CommitNotFound(String),
    #[error("rust-toolchain.toml file not found in the project directory or above it")]
    ToolchainNotFound,
    #[error("rust-toolchain.toml content is not valid: {0}")]
    InvalidToolchain(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0:#?}")]
    Internal(#[from] anyhow::Error),
}

/// A pinned image holding cargo-stylus versions and Rust toolchains, each toolchain with the
/// wasm target and rust-src, and `git`. A build runs the cargo-stylus version that
/// `CARGO_STYLUS_VERSION` names, with the toolchain the project's `rust-toolchain.toml` names.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolImage {
    pub image: String,
    pub cargo_stylus_versions: Vec<Version>,
    pub toolchains: Vec<Version>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct VerifierSettings {
    /// A cargo-stylus version is supported when an image carries it. An environment variable can
    /// hold the list as a JSON string.
    #[serde_as(as = "PickFirst<(_, JsonString)>")]
    pub images: Vec<ToolImage>,
    /// Builds are heavy; requests beyond this wait.
    pub max_concurrent_verifications: NonZeroUsize,
    /// For local chains in development. Leave off in production: the endpoint comes from the
    /// request.
    pub allow_private_rpc_endpoints: bool,
    pub rpc_timeout_seconds: u64,
    pub job: JobSettings,
}

impl Default for VerifierSettings {
    fn default() -> Self {
        Self {
            images: Vec::new(),
            max_concurrent_verifications: NonZeroUsize::new(2).expect("non-zero"),
            allow_private_rpc_endpoints: false,
            rpc_timeout_seconds: 15,
            job: JobSettings::default(),
        }
    }
}

/// Verifies contracts by building them in a sandbox on the remote Docker host and comparing the
/// result with the deployment transaction here, outside it.
pub struct Verifier {
    executor: DockerCompilerExecutor,
    settings: VerifierSettings,
    admission: Semaphore,
}

impl Verifier {
    pub fn new(
        executor: DockerCompilerExecutor,
        settings: VerifierSettings,
    ) -> Result<Self, anyhow::Error> {
        settings
            .job
            .sandbox()
            .validate()
            .context("invalid sandbox settings")?;
        if let Some(image) = settings
            .images
            .iter()
            .find(|image| image.cargo_stylus_versions.is_empty() || image.toolchains.is_empty())
        {
            anyhow::bail!(
                "image {} needs at least one cargo-stylus version and one toolchain",
                image.image
            );
        }
        Ok(Self {
            admission: Semaphore::new(settings.max_concurrent_verifications.get()),
            executor,
            settings,
        })
    }

    pub fn supported_cargo_stylus_versions(&self) -> Vec<Version> {
        let versions: BTreeSet<_> = self
            .settings
            .images
            .iter()
            .flat_map(|image| image.cargo_stylus_versions.iter().cloned())
            .collect();
        versions.into_iter().collect()
    }

    pub async fn verify_github_repository(
        &self,
        request: VerifyGithubRepositoryRequest,
    ) -> Result<Success, Error> {
        let repository_url = validate_repository_url(&request.repository_url)?;
        validate_commit(&request.commit)?;
        let path_prefix = validate_path_prefix(&request.path_prefix)?;
        let images: Vec<&ToolImage> = self
            .settings
            .images
            .iter()
            .filter(|image| {
                image
                    .cargo_stylus_versions
                    .contains(&request.cargo_stylus_version)
            })
            .collect();
        if images.is_empty() {
            return Err(Error::BadRequest(format!(
                "cargo stylus version is not supported: {}",
                request.cargo_stylus_version
            )));
        }

        let _permit = self
            .admission
            .acquire()
            .await
            .context("verification admission is closed")?;
        let deployment = deployment::fetch(
            &request.rpc_endpoint,
            &request.deployment_transaction,
            self.settings.allow_private_rpc_endpoints,
            Duration::from_secs(self.settings.rpc_timeout_seconds),
        )
        .await?;
        if !deployment.creates_contract {
            return Err(Error::VerificationFailed(
                "the deployment transaction does not create a contract".to_string(),
            ));
        }

        let mut session = self
            .executor
            .start_sandbox(self.settings.job.sandbox())
            .await
            .map_err(internal)?;
        let built = self
            .build_in_sandbox(
                &mut session,
                &images,
                &request.cargo_stylus_version,
                &repository_url,
                &request.commit,
                &path_prefix,
            )
            .await;
        session.finish().await;
        let built = built?;

        // The comparison happens here, on bytes the sandbox cannot influence after the fact.
        if built.initcode != deployment.input {
            return Err(Error::VerificationFailed(
                "the contract built from the repository does not match the deployment transaction"
                    .to_string(),
            ));
        }
        Ok(Success {
            abi: built.abi,
            contract_name: built.contract_name,
            files: built.files,
            package_name: built.package_name,
            cargo_stylus_version: request.cargo_stylus_version,
            repository_url: request.repository_url,
            commit: request.commit,
            path_prefix: request.path_prefix,
        })
    }

    async fn build_in_sandbox(
        &self,
        session: &mut SandboxSession,
        images: &[&ToolImage],
        cargo_stylus_version: &Version,
        repository_url: &str,
        commit: &str,
        path_prefix: &str,
    ) -> Result<Built, Error> {
        let job = &self.settings.job;
        // Every image of the version has git; the toolchain is not known before the clone.
        let cloned = session
            .run_step(&job.clone_step(&images[0].image, repository_url, commit, path_prefix))
            .await
            .map_err(|error| clone_error(error, repository_url, commit))?;
        let toolchain_directories =
            job::toolchain_directories(&String::from_utf8_lossy(&cloned.stdout));
        let package_file = read_text(
            session,
            &job.manifest_output(path_prefix, PACKAGE_FILE_NAME),
        )
        .await?;
        let package_name = parse_package_name(&package_file)?;

        let root = if job::builds_from_workspace_root(cargo_stylus_version) {
            // Any toolchain of the image can read the manifests.
            let located = session
                .run_step(&job.locate_step(&images[0].image, &images[0].toolchains[0], path_prefix))
                .await
                .map_err(build_error)?;
            let workspace_root =
                job::workspace_root(&String::from_utf8_lossy(&located.stdout), path_prefix)
                    .ok_or_else(|| {
                        Error::BadRequest(
                            "the project's workspace root is not inside the repository".to_string(),
                        )
                    })?;
            job::BuildRoot::workspace(workspace_root, package_name.clone())
        } else {
            job::BuildRoot::project(path_prefix)
        };

        let toolchain_directory = root
            .toolchain_directory(&toolchain_directories)
            .ok_or(Error::ToolchainNotFound)?;
        let toolchain_file = read_text(
            session,
            &job.manifest_output(toolchain_directory, TOOLCHAIN_FILE_NAME),
        )
        .await?;
        let toolchain = validate_toolchain_channel(&parse_toolchain_channel(&toolchain_file)?)?;
        let tools = images
            .iter()
            .find(|image| image.toolchains.contains(&toolchain))
            .map(|image| job::Tools {
                image: image.image.clone(),
                cargo_stylus_version: cargo_stylus_version.clone(),
                toolchain: toolchain.clone(),
            })
            .ok_or_else(|| {
                let available: BTreeSet<_> = images
                    .iter()
                    .flat_map(|image| image.toolchains.iter().map(ToString::to_string))
                    .collect();
                Error::InvalidToolchain(format!(
                    "toolchain {toolchain} is not available for cargo-stylus \
                     {cargo_stylus_version}; available: {}",
                    available.into_iter().collect::<Vec<_>>().join(", ")
                ))
            })?;

        session
            .run_step(&job.fetch_step(&tools, path_prefix))
            .await
            .map_err(build_error)?;
        session
            .run_step(&job.build_step(&tools, &root))
            .await
            .map_err(build_error)?;
        let initcode = read_text(session, &job.initcode_output()).await?;
        let initcode = blockscout_display_bytes::decode_hex(initcode.trim())
            .map_err(|error| anyhow!("the build wrote initcode that is not hex: {error}"))?;

        // As before, a contract whose ABI cannot be exported is still verified.
        let exported = match session.run_step(&job.export_abi_step(&tools, &root)).await {
            Ok(output) => process_export_abi_output(&String::from_utf8_lossy(&output.stdout))?,
            Err(
                ExecutionError::CompilerFailed { .. }
                | ExecutionError::Timeout { .. }
                | ExecutionError::OutputLimitExceeded { .. },
            ) => None,
            Err(error) => return Err(internal(error)),
        };
        let (contract_name, abi) = exported.unzip();
        let contract_name = contract_name_or_package(contract_name, &package_name);

        let sources = session
            .read_output(&job.sources_output(path_prefix))
            .await
            .map_err(|error| match error {
                ExecutionError::OutputLimitExceeded { limit } => {
                    Error::BadRequest(format!("project directory exceeds the {limit}-byte limit"))
                }
                error => internal(error),
            })?;

        Ok(Built {
            initcode: initcode.into(),
            abi,
            contract_name,
            files: source_files(sources),
            package_name,
        })
    }
}

struct Built {
    initcode: Bytes,
    abi: Option<serde_json::Value>,
    contract_name: Option<String>,
    files: BTreeMap<String, String>,
    package_name: String,
}

fn internal(error: ExecutionError) -> Error {
    Error::Internal(anyhow::Error::new(error))
}

/// The clone step runs no repository code, so its exit code says what went wrong.
fn clone_error(error: ExecutionError, repository_url: &str, commit: &str) -> Error {
    let ExecutionError::CompilerFailed { exit_code, .. } = &error else {
        return internal(error);
    };
    match *exit_code {
        clone_exit::TOOLCHAIN_NOT_FOUND => Error::ToolchainNotFound,
        clone_exit::COMMIT_NOT_FOUND => Error::CommitNotFound(commit.to_string()),
        clone_exit::REPOSITORY_NOT_FOUND => Error::RepositoryNotFound(repository_url.to_string()),
        clone_exit::INVALID_PATH_PREFIX => Error::BadRequest(
            "path_prefix must point to a directory inside the repository".to_string(),
        ),
        clone_exit::PACKAGE_NOT_FOUND => {
            Error::BadRequest("Cargo.toml file not found in project directory".to_string())
        }
        clone_exit::INVALID_TARGET => Error::BadRequest(
            "every `target` from the repository root to the project must be a directory"
                .to_string(),
        ),
        _ => internal(error),
    }
}

/// Failures of the steps that run the project's own code are the project's failures.
fn build_error(error: ExecutionError) -> Error {
    match error {
        ExecutionError::CompilerFailed {
            oom_killed: true, ..
        } => Error::VerificationFailed("the build ran out of memory".to_string()),
        ExecutionError::CompilerFailed { stderr, .. } => {
            Error::VerificationFailed(tail(&stderr, MAX_FAILURE_DETAILS_BYTES).to_string())
        }
        ExecutionError::Timeout { seconds } => Error::VerificationFailed(format!(
            "the build exceeded its {seconds}-second time limit"
        )),
        ExecutionError::OutputLimitExceeded { .. } => {
            Error::VerificationFailed("the build produced too much output".to_string())
        }
        error => internal(error),
    }
}

fn tail(text: &str, max_bytes: usize) -> &str {
    let mut start = text.len().saturating_sub(max_bytes);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

async fn read_text(session: &mut SandboxSession, output: &SandboxOutput) -> Result<String, Error> {
    let files = session.read_output(output).await.map_err(internal)?;
    let [file] = files.as_slice() else {
        return Err(anyhow!("expected one regular file at {}", output.path).into());
    };
    String::from_utf8(file.content.to_vec())
        .map_err(|_| Error::BadRequest(format!("{} is not valid UTF-8", output.path)))
}

/// Only github.com over https, and only the plain `owner/repository` form: the URL is handed to
/// git inside the sandbox.
fn validate_repository_url(url: &Url) -> Result<String, Error> {
    let not_github = || Error::RepositoryIsNotGithub(url.to_string());
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(not_github());
    }
    let segments: Vec<&str> = url.path().trim_matches('/').split('/').collect();
    let is_name = |segment: &&str| {
        !segment.is_empty()
            && !matches!(*segment, "." | "..")
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    };
    match segments.as_slice() {
        [owner, repository] if is_name(owner) && is_name(repository) => {
            Ok(format!("https://github.com/{owner}/{repository}"))
        }
        _ => Err(not_github()),
    }
}

fn validate_commit(commit: &str) -> Result<(), Error> {
    if (7..=40).contains(&commit.len()) && commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(Error::CommitNotFound(commit.to_string()))
    }
}

/// A relative path with no parent components, in the form the sandbox steps expect: `a/b`, or
/// empty for the repository root.
fn validate_path_prefix(path_prefix: &Path) -> Result<String, Error> {
    let invalid = || {
        Error::BadRequest(
            "path_prefix must be a relative path without parent directory components".to_string(),
        )
    };
    let mut components = Vec::new();
    for component in path_prefix.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(name) => {
                let name = name.to_str().ok_or_else(invalid)?;
                if name.chars().any(char::is_control) {
                    return Err(invalid());
                }
                components.push(name);
            }
            _ => return Err(invalid()),
        }
    }
    Ok(components.join("/"))
}

fn parse_toolchain_channel(toolchain_file_contents: &str) -> Result<String, Error> {
    let toolchain_toml: toml::Value = toml::from_str(toolchain_file_contents).map_err(|err| {
        Error::InvalidToolchain(format!("failed to parse rust-toolchain.toml; {err}"))
    })?;

    // Extract the channel from the toolchain section
    let Some(toolchain) = toolchain_toml.get("toolchain") else {
        return Err(Error::InvalidToolchain(
            "toolchain section not found in rust-toolchain.toml".into(),
        ));
    };
    let Some(channel) = toolchain.get("channel") else {
        return Err(Error::InvalidToolchain(
            "could not find channel in rust-toolchain.toml's toolchain section".into(),
        ));
    };
    let Some(channel) = channel.as_str() else {
        return Err(Error::InvalidToolchain(
            "channel in rust-toolchain.toml's toolchain section is not a string".into(),
        ));
    };

    // Reject "stable" and "nightly" channels specified alone
    if channel == "stable" || channel == "nightly" || channel == "beta" {
        return Err(Error::InvalidToolchain(
            "the channel in your project's rust-toolchain.toml's toolchain section must be a specific version e.g., '1.80.0' or 'nightly-YYYY-MM-DD'. \
            To ensure reproducibility, it cannot be a generic channel like 'stable', 'nightly', or 'beta'".into()
        ));
    }

    // Parse the Rust version from the toolchain project, only allowing alphanumeric chars and dashes.
    let channel = channel
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '.')
        .collect();

    Ok(channel)
}

fn validate_toolchain_channel(channel: &str) -> Result<Version, Error> {
    if channel.contains("nightly") {
        return Err(Error::InvalidToolchain(
            "nightly channel are currently not supported for verification".into(),
        ));
    }

    let version = Version::parse(channel).map_err(|err| {
        Error::InvalidToolchain(format!(
            "failed to parse toolchain version from rust-toolchain.toml as semver::Version; {err}"
        ))
    })?;

    Ok(version)
}

/// The manifest comes from the repository, so what is wrong with it is the request's fault. The
/// name is passed to `cargo stylus --contract`, so it has to be one cargo accepts.
fn parse_package_name(package_file_contents: &str) -> Result<String, Error> {
    let invalid = |reason: &str| Error::BadRequest(format!("project's Cargo.toml: {reason}"));
    let cargo_toml: toml::Value = toml::from_str(package_file_contents)
        .map_err(|error| invalid(&format!("cannot be parsed: {error}")))?;
    let package_name = cargo_toml
        .get("package")
        .ok_or_else(|| invalid("has no package section"))?
        .get("name")
        .ok_or_else(|| invalid("has no package name"))?
        .as_str()
        .ok_or_else(|| invalid("package name is not a string"))?;
    let is_valid = package_name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && package_name
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '_' | '-'));
    if !is_valid {
        return Err(invalid("package name is not a valid crate name"));
    }
    Ok(package_name.to_string())
}

/// The contract name comes from the exported ABI, which some contracts do not print; explorers
/// need a name, so the package's stands in.
fn contract_name_or_package(contract_name: Option<String>, package_name: &str) -> Option<String> {
    contract_name.or_else(|| Some(package_name.to_string()))
}

/// The project's manifests and Rust sources, keyed by their path inside the project.
fn source_files(files: Vec<SandboxFile>) -> BTreeMap<String, String> {
    files
        .into_iter()
        .filter_map(|file| {
            // The archive names entries `<directory>/<path>`; the directory is the project root.
            let (_, path) = file.path.split_once('/')?;
            let mut components: Vec<&str> = path.split('/').collect();
            let name = components.pop()?;
            let in_skipped_directory = components
                .iter()
                .any(|directory| matches!(*directory, "target" | ".git"));
            let wanted = matches!(name, "rust-toolchain.toml" | "Cargo.toml" | "Cargo.lock")
                || name.ends_with(".rs");
            (wanted && !in_skipped_directory).then(|| {
                (
                    path.to_string(),
                    String::from_utf8_lossy(&file.content).into_owned(),
                )
            })
        })
        .collect()
}

fn process_export_abi_output(output: &str) -> Result<Option<(String, serde_json::Value)>, Error> {
    let mut contract_names = Vec::new();
    let mut signatures = HashSet::new();

    let mut lines = output.lines();
    while let Some(name) = skip_till_next_interface(&mut lines) {
        let items = process_interface(&mut lines);
        contract_names.push(name);
        signatures.extend(items);
    }

    let out = contract_names
        .drain(..)
        .next_back()
        .map(|name| {
            let json_abi = JsonAbi::parse(signatures).context("failed to parse json abi")?;
            let value = serde_json::to_value(json_abi).context("failed to serialize json abi")?;
            Ok((name, value))
        })
        .transpose();
    out
}

fn skip_till_next_interface(lines: &mut Lines) -> Option<String> {
    for line in lines {
        if !line.trim().starts_with("interface I") {
            continue;
        }

        return Some(
            line.trim_start_matches("interface I")
                .chars()
                .take_while(|c| *c != ' ')
                .collect(),
        );
    }
    None
}

fn process_interface<'a>(lines: &mut Lines<'a>) -> Vec<&'a str> {
    lines
        .take_while(|&line| !line.starts_with('}'))
        .filter(|&line| !line.is_empty())
        .map(|line| line.trim().trim_end_matches(';'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn file(path: &str, content: &str) -> SandboxFile {
        SandboxFile {
            path: path.to_string(),
            content: Bytes::copy_from_slice(content.as_bytes()),
        }
    }

    #[test]
    fn repository_url_must_be_a_plain_github_repository() {
        for (url, expected) in [
            (
                "https://github.com/blockscout/.github",
                Some("https://github.com/blockscout/.github"),
            ),
            (
                "https://github.com/OffchainLabs/stylus-hello-world",
                Some("https://github.com/OffchainLabs/stylus-hello-world"),
            ),
            (
                "https://github.com/a_b/c.d-e/",
                Some("https://github.com/a_b/c.d-e"),
            ),
            ("http://github.com/a/b", None),
            ("https://gitlab.com/a/b", None),
            ("https://github.com.example.org/a/b", None),
            ("https://user:token@github.com/a/b", None),
            ("https://github.com:8443/a/b", None),
            ("https://github.com/a/b?x=1", None),
            ("https://github.com/a/b#x", None),
            ("https://github.com/a", None),
            ("https://github.com/a/b/c", None),
            ("https://github.com/a/..", None),
            ("https://github.com/a/b%20c", None),
            ("https://github.com/a/b/", Some("https://github.com/a/b")),
        ] {
            let result = validate_repository_url(&Url::parse(url).unwrap());
            match expected {
                Some(expected) => assert_eq!(result.unwrap(), expected, "{url}"),
                None => assert!(
                    matches!(result, Err(Error::RepositoryIsNotGithub(_))),
                    "{url} was accepted"
                ),
            }
        }
    }

    #[test]
    fn commit_must_be_a_hash() {
        for commit in [
            "af5029f",
            "af5029f822815e32def0015bf8e591e769c62f34",
            "ABCDEF0",
        ] {
            validate_commit(commit).unwrap();
        }
        for commit in [
            "",
            "abc",
            "qwertyu",
            "--help",
            "main",
            "af5029f822815e32def0015bf8e591e769c62f34a",
        ] {
            assert!(
                matches!(validate_commit(commit), Err(Error::CommitNotFound(_))),
                "{commit:?} was accepted"
            );
        }
    }

    #[test]
    fn path_prefix_stays_inside_the_repository() {
        for (prefix, expected) in [
            ("", ""),
            ("examples/single_call", "examples/single_call"),
            ("./examples/single_call/", "examples/single_call"),
            ("with space/x", "with space/x"),
        ] {
            assert_eq!(validate_path_prefix(Path::new(prefix)).unwrap(), expected);
        }
        for prefix in ["..", "../outside", "a/../../b", "/etc", "a/\nb"] {
            assert!(
                matches!(
                    validate_path_prefix(Path::new(prefix)),
                    Err(Error::BadRequest(_))
                ),
                "{prefix:?} was accepted"
            );
        }
    }

    #[test]
    fn toolchain_channel_is_read_from_the_manifest() {
        assert_eq!(
            parse_toolchain_channel("[toolchain]\nchannel = \"1.80.0\"\n").unwrap(),
            "1.80.0"
        );
        for contents in [
            "",
            "[toolchain]\n",
            "not toml [",
            "[toolchain]\nchannel = \"stable\"\n",
        ] {
            assert!(
                matches!(
                    parse_toolchain_channel(contents),
                    Err(Error::InvalidToolchain(_))
                ),
                "{contents:?} was accepted"
            );
        }
        assert!(matches!(
            validate_toolchain_channel("nightly-2024-01-01"),
            Err(Error::InvalidToolchain(_))
        ));
        assert_eq!(
            validate_toolchain_channel("1.80.0").unwrap(),
            Version::new(1, 80, 0)
        );
    }

    #[test]
    fn package_name_stands_in_for_a_missing_contract_name() {
        assert_eq!(
            contract_name_or_package(Some("SingleCall".to_string()), "stylus-single-call"),
            Some("SingleCall".to_string())
        );
        assert_eq!(
            contract_name_or_package(None, "vess_contract"),
            Some("vess_contract".to_string())
        );
    }

    #[test]
    fn package_name_is_read_from_the_manifest() {
        assert_eq!(
            parse_package_name("[package]\nname = \"stylus-single-call\"\n").unwrap(),
            "stylus-single-call"
        );
        for manifest in [
            "[workspace]\n",
            "not toml",
            "[package]\nname = 1\n",
            "[package]\nname = \"--contract\"\n",
            "[package]\nname = \"a b\"\n",
            "[package]\nname = \"\"\n",
        ] {
            assert!(
                matches!(parse_package_name(manifest), Err(Error::BadRequest(_))),
                "{manifest:?}"
            );
        }
    }

    #[test]
    fn response_holds_manifests_and_rust_sources_of_the_project_only() {
        let files = source_files(vec![
            file("single_call/Cargo.toml", "[package]"),
            file("single_call/Cargo.lock", "lock"),
            file("single_call/rust-toolchain.toml", "[toolchain]"),
            file("single_call/src/lib.rs", "pub fn f() {}"),
            file("single_call/README.md", "readme"),
            file("single_call/scripts/main.js", "js"),
            file("single_call/target/debug/build/out.rs", "generated"),
            file("single_call/.git/hooks/x.rs", "hook"),
            file("single_call", "a file where the directory should be"),
        ]);
        assert_eq!(
            files.keys().collect::<Vec<_>>(),
            [
                "Cargo.lock",
                "Cargo.toml",
                "rust-toolchain.toml",
                "src/lib.rs"
            ]
        );
        assert_eq!(files["src/lib.rs"], "pub fn f() {}");
    }

    #[test]
    fn clone_exit_codes_become_request_errors() {
        let failed = |exit_code| ExecutionError::CompilerFailed {
            compiler: "clone".to_string(),
            exit_code,
            stderr: String::new(),
            oom_killed: false,
        };
        let error = |exit_code| clone_error(failed(exit_code), "https://github.com/a/b", "abcdef0");
        assert!(matches!(error(64), Error::ToolchainNotFound));
        assert!(matches!(error(65), Error::CommitNotFound(commit) if commit == "abcdef0"));
        assert!(matches!(error(66), Error::RepositoryNotFound(_)));
        assert!(matches!(error(67), Error::BadRequest(_)));
        assert!(matches!(error(68), Error::BadRequest(_)));
        assert!(matches!(error(69), Error::BadRequest(_)));
        assert!(matches!(error(1), Error::Internal(_)));
        assert!(matches!(
            clone_error(ExecutionError::Timeout { seconds: 5 }, "u", "c"),
            Error::Internal(_)
        ));
    }

    #[test]
    fn build_failures_are_verification_failures_with_the_end_of_the_output() {
        let stderr = format!(
            "{}error: could not compile",
            "x".repeat(MAX_FAILURE_DETAILS_BYTES)
        );
        let error = build_error(ExecutionError::CompilerFailed {
            compiler: "build".to_string(),
            exit_code: 101,
            stderr,
            oom_killed: false,
        });
        match error {
            Error::VerificationFailed(details) => {
                assert!(details.ends_with("error: could not compile"));
                assert_eq!(details.len(), MAX_FAILURE_DETAILS_BYTES);
            }
            other => panic!("unexpected error: {other}"),
        }
        assert!(matches!(
            build_error(ExecutionError::Timeout { seconds: 900 }),
            Error::VerificationFailed(_)
        ));
        assert!(matches!(
            build_error(ExecutionError::InvalidInvocation("x".to_string())),
            Error::Internal(_)
        ));
        assert_eq!(tail("aé", 1), "");
    }

    #[test]
    fn parse_export_abi_output_single_interface() {
        let expected_name = "Counter";

        // Currently we do not support returning `internalType`s in the ABI.
        // For testing purposes expected abi was removed of `internalType`s then.
        //
        // The original expected abi:
        // [{"inputs":[{"internalType":"uint256","name":"new_number","type":"uint256"}],"name":"addNumber","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"increment","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"uint256","name":"new_number","type":"uint256"}],"name":"mulNumber","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"number","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"new_number","type":"uint256"}],"name":"setNumber","outputs":[],"stateMutability":"nonpayable","type":"function"}]
        let expected_abi = serde_json::json!([{"inputs":[{"name":"new_number","type":"uint256"}],"name":"addNumber","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"increment","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"name":"new_number","type":"uint256"}],"name":"mulNumber","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"number","outputs":[{"name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"name":"new_number","type":"uint256"}],"name":"setNumber","outputs":[],"stateMutability":"nonpayable","type":"function"}]);

        let export_abi_output = r#"
/**
 * This file was automatically generated by Stylus and represents a Rust program.
 * For more information, please see [The Stylus SDK](https://github.com/OffchainLabs/stylus-sdk-rs).
 */

// SPDX-License-Identifier: MIT-OR-APACHE-2.0
pragma solidity ^0.8.23;

interface ICounter {
    function number() external view returns (uint256);

    function setNumber(uint256 new_number) external;

    function mulNumber(uint256 new_number) external;

    function addNumber(uint256 new_number) external;

    function increment() external;
}"#;

        let (name, abi) = process_export_abi_output(export_abi_output)
            .expect("function failed")
            .expect("no interface was found");
        assert_eq!(name, expected_name, "invalid interface name");
        assert_eq!(abi, expected_abi, "invalid json abi");
    }

    #[test]
    fn parse_export_abi_output_multiple_interfaces() {
        let expected_name = "StylusTestToken";

        // Original (`internalType`s were removed, "error"s were moved in the end):
        // [{"inputs":[{"internalType":"address","name":"","type":"address"},{"internalType":"address","name":"","type":"address"},{"internalType":"uint256","name":"","type":"uint256"},{"internalType":"uint256","name":"","type":"uint256"}],"name":"InsufficientAllowance","type":"error"},{"inputs":[{"internalType":"address","name":"","type":"address"},{"internalType":"uint256","name":"","type":"uint256"},{"internalType":"uint256","name":"","type":"uint256"}],"name":"InsufficientBalance","type":"error"},{"inputs":[{"internalType":"address","name":"owner","type":"address"},{"internalType":"address","name":"spender","type":"address"}],"name":"allowance","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"spender","type":"address"},{"internalType":"uint256","name":"value","type":"uint256"}],"name":"approve","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"owner","type":"address"}],"name":"balanceOf","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"uint256","name":"value","type":"uint256"}],"name":"burn","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"decimals","outputs":[{"internalType":"uint8","name":"","type":"uint8"}],"stateMutability":"pure","type":"function"},{"inputs":[{"internalType":"uint256","name":"value","type":"uint256"}],"name":"mint","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"to","type":"address"},{"internalType":"uint256","name":"value","type":"uint256"}],"name":"mintTo","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"name","outputs":[{"internalType":"string","name":"","type":"string"}],"stateMutability":"pure","type":"function"},{"inputs":[],"name":"symbol","outputs":[{"internalType":"string","name":"","type":"string"}],"stateMutability":"pure","type":"function"},{"inputs":[],"name":"totalSupply","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"address","name":"to","type":"address"},{"internalType":"uint256","name":"value","type":"uint256"}],"name":"transfer","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"from","type":"address"},{"internalType":"address","name":"to","type":"address"},{"internalType":"uint256","name":"value","type":"uint256"}],"name":"transferFrom","outputs":[{"internalType":"bool","name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"}]
        let expected_abi = serde_json::json!([{"inputs":[{"name":"owner","type":"address"},{"name":"spender","type":"address"}],"name":"allowance","outputs":[{"name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"name":"spender","type":"address"},{"name":"value","type":"uint256"}],"name":"approve","outputs":[{"name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"name":"owner","type":"address"}],"name":"balanceOf","outputs":[{"name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"name":"value","type":"uint256"}],"name":"burn","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"decimals","outputs":[{"name":"","type":"uint8"}],"stateMutability":"pure","type":"function"},{"inputs":[{"name":"value","type":"uint256"}],"name":"mint","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"name":"mintTo","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"name","outputs":[{"name":"","type":"string"}],"stateMutability":"pure","type":"function"},{"inputs":[],"name":"symbol","outputs":[{"name":"","type":"string"}],"stateMutability":"pure","type":"function"},{"inputs":[],"name":"totalSupply","outputs":[{"name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"name":"transfer","outputs":[{"name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"name":"from","type":"address"},{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"name":"transferFrom","outputs":[{"name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"name":"","type":"address"},{"name":"","type":"address"},{"name":"","type":"uint256"},{"name":"","type":"uint256"}],"name":"InsufficientAllowance","type":"error"},{"inputs":[{"name":"","type":"address"},{"name":"","type":"uint256"},{"name":"","type":"uint256"}],"name":"InsufficientBalance","type":"error"}]);

        let export_abi_output = r#"
/**
 * This file was automatically generated by Stylus and represents a Rust program.
 * For more information, please see [The Stylus SDK](https://github.com/OffchainLabs/stylus-sdk-rs).
 */

// SPDX-License-Identifier: MIT-OR-APACHE-2.0
pragma solidity ^0.8.23;

interface IErc20 {
    function name() external pure returns (string memory);

    function symbol() external pure returns (string memory);

    function decimals() external pure returns (uint8);

    function totalSupply() external view returns (uint256);

    function balanceOf(address owner) external view returns (uint256);

    function transfer(address to, uint256 value) external returns (bool);

    function transferFrom(address from, address to, uint256 value) external returns (bool);

    function approve(address spender, uint256 value) external returns (bool);

    function allowance(address owner, address spender) external view returns (uint256);

    error InsufficientBalance(address, uint256, uint256);

    error InsufficientAllowance(address, address, uint256, uint256);
}

interface IStylusTestToken is IErc20 {
    function mint(uint256 value) external;

    function mintTo(address to, uint256 value) external;

    function burn(uint256 value) external;

    error InsufficientBalance(address, uint256, uint256);

    error InsufficientAllowance(address, address, uint256, uint256);
}
        "#;

        let (name, abi) = process_export_abi_output(export_abi_output)
            .expect("function failed")
            .expect("no interface was found");
        assert_eq!(name, expected_name, "invalid interface name");
        assert_eq!(abi, expected_abi, "invalid json abi");
    }
}
