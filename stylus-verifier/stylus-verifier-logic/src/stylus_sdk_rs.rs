use crate::docker;
use alloy_json_abi::JsonAbi;
use anyhow::{anyhow, Context};
use blockscout_display_bytes::ToHex;
use bytes::Bytes;
use git2::Repository;
use semver::Version;
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    str::Lines,
};
use tempfile::TempDir;
use thiserror::Error;
use tokio::fs;
use url::Url;

/// Name of the toolchain file used to specify the Rust toolchain version for a project.
pub const TOOLCHAIN_FILE_NAME: &str = "rust-toolchain.toml";

/// The last line to be expected from the `cargo stylus verify` command when the contract is verified.
pub const CONTRACT_VERIFIED_MESSAGE: &str =
    "Verified - contract matches local project's file hashes";

/// The line to be expected from the `cargo stylus verify` command when the contract verification fails.
pub const VERIFICATION_FAILED_MESSAGE: &str =
    "contract deployment did not verify against local project's file hashes";

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
    #[error("rust-toolchain.toml file not found in project directory")]
    ToolchainNotFound,
    #[error("rust-toolchain.toml content is not valid: {0}")]
    InvalidToolchain(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0:#?}")]
    Internal(#[from] anyhow::Error),
}

pub async fn verify_github_repository(
    request: VerifyGithubRepositoryRequest,
) -> Result<Success, Error> {
    let repo_directory =
        github_repository_clone_and_checkout(&request.repository_url, &request.commit).await?;

    let project_path = repo_directory
        .path()
        .to_path_buf()
        .join(&request.path_prefix);

    let toolchain_channel = extract_toolchain_channel(&project_path).await?;
    let toolchain = validate_toolchain_channel(&toolchain_channel)?;

    let verify_output = docker::run_reproducible(
        &request.cargo_stylus_version,
        &toolchain,
        &project_path,
        &[
            "verify",
            "--no-verify",
            "--endpoint",
            &request.rpc_endpoint,
            "--deployment-tx",
            &ToHex::to_hex(&request.deployment_transaction),
        ],
    )
    .await?;

    // TODO: What if rust toolchain would be invalid (non-existent)?

    if verify_output.lines().last().map(|v| v.trim()) != Some(CONTRACT_VERIFIED_MESSAGE) {
        let fail_message_details = verify_output
            .lines()
            .skip_while(|&line| !line.contains(VERIFICATION_FAILED_MESSAGE))
            .skip(1)
            .collect::<Vec<_>>()
            .join("\n");

        return Err(Error::VerificationFailed(fail_message_details));
    }

    let export_abi_output = docker::run_reproducible(
        &request.cargo_stylus_version,
        &toolchain,
        &project_path,
        &["export-abi"],
    )
    .await?;

    let (contract_name, abi) = match process_export_abi_output(&export_abi_output)? {
        Some((name, abi)) => (Some(name), Some(abi)),
        None => (None, None),
    };

    let files = retrieve_source_files(&project_path).await?;

    Ok(Success {
        abi,
        contract_name,
        files,
        cargo_stylus_version: request.cargo_stylus_version,
        repository_url: request.repository_url,
        commit: request.commit,
        path_prefix: request.path_prefix,
    })
}

async fn github_repository_clone_and_checkout(
    repository_url: &Url,
    commit: &str,
) -> Result<TempDir, Error> {
    if repository_url.scheme() != "https" || repository_url.host_str() != Some("github.com") {
        return Err(Error::RepositoryIsNotGithub(repository_url.to_string()));
    }

    let tempdir = tempfile::tempdir().context("failed to create temporary directory")?;

    let repo = match Repository::clone(repository_url.as_str(), tempdir.path()) {
        Ok(repo) => repo,
        Err(err) if err.code() == git2::ErrorCode::Auth => {
            return Err(Error::RepositoryNotFound(repository_url.to_string()));
        }
        Err(err) => {
            return Err(err).context("failed to clone repository")?;
        }
    };

    let commit_object = match repo.revparse_single(commit) {
        Ok(commit_object) => commit_object,
        Err(err) if err.code() == git2::ErrorCode::NotFound => {
            return Err(Error::CommitNotFound(commit.to_string()))
        }
        Err(err) => return Err(err).context("failed to parse commit hash")?,
    };
    repo.checkout_tree(&commit_object, None)
        .context("failed to checkout commit object")?;

    Ok(tempdir)
}

async fn extract_toolchain_channel(directory: &Path) -> Result<String, Error> {
    let toolchain_file_path = directory.join(TOOLCHAIN_FILE_NAME);

    let toolchain_file_contents = match fs::read_to_string(toolchain_file_path).await {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::ToolchainNotFound);
        }
        Err(err) => Err(err).context("failed to read rust-toolchain.toml file")?,
    };

    let toolchain_toml: toml::Value = toml::from_str(&toolchain_file_contents).map_err(|err| {
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

async fn retrieve_source_files(root_dir: &Path) -> Result<BTreeMap<String, String>, Error> {
    let mut files = BTreeMap::new();
    let mut directories = Vec::<PathBuf>::new();
    directories.push(root_dir.to_path_buf()); // Using `from` directly

    while let Some(dir) = directories.pop() {
        for entry in
            std::fs::read_dir(&dir).map_err(|e| anyhow!("failed to read directory {dir:?}: {e}"))?
        {
            let entry = entry.map_err(|e| anyhow!("failed to list entries in {dir:?}: {e}"))?;
            let path = entry.path();

            if path.is_dir() {
                if path.ends_with("target") || path.ends_with(".git") {
                    continue; // Skip "target" and ".git" directories
                }
                directories.push(path);
            } else if path.file_name().map_or(false, |f| {
                // By default include `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, and `.rs` files.
                f == "rust-toolchain.toml"
                    || f == "Cargo.toml"
                    || f == "Cargo.lock"
                    || f.to_string_lossy().ends_with(".rs")
            }) {
                let file_path = path
                    .strip_prefix(root_dir)
                    .expect("path got as a result of 'root_dir' iterating")
                    .to_string_lossy()
                    .to_string();
                let content = fs::read_to_string(&path)
                    .await
                    .map_err(|e| anyhow!("failed to read file {path:?}: {e}"))?;
                files.insert(file_path, content);
            }
        }
    }
    Ok(files)
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

    contract_names
        .drain(..)
        .last()
        .map(|name| {
            let json_abi = JsonAbi::parse(signatures).context("failed to parse json abi")?;
            let value = serde_json::to_value(json_abi).context("failed to serialize json abi")?;
            Ok((name, value))
        })
        .transpose()
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
    use std::ffi::OsStr;

    #[tokio::test]
    async fn github_repository_clone_and_checkout_success() {
        let repository_url = Url::parse("https://github.com/blockscout/.github").unwrap();
        let commit = "703f8e1";
        let directory = github_repository_clone_and_checkout(&repository_url, commit)
            .await
            .expect("failed to clone repository");

        // there should be only `README.md file` and `.git` directory
        let mut count = 0;
        for entry in directory
            .path()
            .read_dir()
            .expect("failed to read directory")
        {
            let entry = entry.expect("failed to read entry");
            let path = entry.path();
            match path.file_name() {
                Some(name) if name == OsStr::new("README.md") && path.is_file() => {
                    let content = fs::read_to_string(path)
                        .await
                        .expect("failed to read README.md content");
                    assert_eq!(content, "# .github", "invalid README.md file content");
                }
                Some(name) if name == OsStr::new(".git") && path.is_dir() => {}
                _ => panic!(
                    "unexpected entry (is_directory={}): {path:?}",
                    path.is_dir()
                ),
            }
            count += 1;
        }
        assert_eq!(count, 2, "invalid number of entries");
    }

    #[tokio::test]
    async fn invalid_commit_hash() {
        let repository_url = Url::parse("https://github.com/blockscout/.github").unwrap();

        // Non-existent commit hash
        let commit = "0000000";
        let err = github_repository_clone_and_checkout(&repository_url, commit)
            .await
            .expect_err("(non-existent) error expected");
        assert!(
            matches!(err, Error::CommitNotFound(_)),
            "(non-existent) invalid error: {err}"
        );

        // invalid hex value commit hash
        let commit = "qwerty";
        let err = github_repository_clone_and_checkout(&repository_url, commit)
            .await
            .expect_err("(invalid hex) error expected");
        assert!(
            matches!(err, Error::CommitNotFound(_)),
            "(invalid hex) invalid error: {err}"
        );

        // too long value commit hash (21 bytes)
        let commit = "0123456789012345678901234567890123456789ab";
        let err = github_repository_clone_and_checkout(&repository_url, commit)
            .await
            .expect_err("(too long) error expected");
        assert!(
            matches!(err, Error::CommitNotFound(_)),
            "(too long) invalid error: {err}"
        );
    }

    #[tokio::test]
    async fn invalid_repository_url() {
        // Non-existent repository inside organization
        let repository_url = Url::parse("https://github.com/blockscout/nonexistentrepo").unwrap();
        let commit = "703f8e1";
        let err = github_repository_clone_and_checkout(&repository_url, commit)
            .await
            .expect_err("(non-existent repository) error expected");
        assert!(
            matches!(err, Error::RepositoryNotFound(_)),
            "(non-existent repository) invalid error: {err}"
        );

        // Non-existent organization
        let repository_url = Url::parse("https://github.com/nonexistentorg/.github").unwrap();
        let commit = "703f8e1";
        let err = github_repository_clone_and_checkout(&repository_url, commit)
            .await
            .expect_err("(non-existent organization) error expected");
        assert!(
            matches!(err, Error::RepositoryNotFound(_)),
            "(non-existent commit organization) invalid error: {err}"
        );

        // Not github url
        let repository_url = Url::parse("https://notgithub.com/blockscout/.github").unwrap();
        let commit = "703f8e1";
        let err = github_repository_clone_and_checkout(&repository_url, commit)
            .await
            .expect_err("(not github url) error expected");
        assert!(
            matches!(err, Error::RepositoryIsNotGithub(_)),
            "(not github url) invalid error: {err}"
        );
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
