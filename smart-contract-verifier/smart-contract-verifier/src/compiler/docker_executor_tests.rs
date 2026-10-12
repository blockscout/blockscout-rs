// SPDX-License-Identifier: LicenseRef-Blockscout

//! End-to-end check of the Docker executor with a real compiler, which needs this crate's fetcher.

use crate::{
    CommandArgument, CompilerExecutor, CompilerInvocation, DetailedVersion, DockerCompilerExecutor,
    DockerCompilerExecutorSettings, Fetcher, JobFile, ListFetcher,
};
use bytes::Bytes;
use std::{path::PathBuf, str::FromStr};
use url::Url;

const LINUX_AMD64_VYPER_COMPILER_LIST: &str =
    "https://raw.githubusercontent.com/blockscout/solc-bin/main/vyper.list.json";

#[tokio::test]
#[ignore = "requires remote Docker test settings and network access"]
async fn ssh_docker_vyper_round_trip() {
    let settings: DockerCompilerExecutorSettings = serde_json::from_value(serde_json::json!({
        "addr": std::env::var("SCV_TEST_DOCKER_ADDR").unwrap(),
        "key_path": std::env::var_os("SCV_TEST_DOCKER_KEY_PATH").map(PathBuf::from),
        "runner_image": std::env::var("SCV_TEST_DOCKER_RUNNER_IMAGE").unwrap(),
        "platform": std::env::var("SCV_TEST_DOCKER_PLATFORM")
            .unwrap_or_else(|_| "linux/amd64".to_string()),
        "connect_timeout_seconds": 10,
        "api_timeout_seconds": 10,
        "execution_timeout_seconds": 30,
        "memory_limit_bytes": 512 * 1024 * 1024,
        "nano_cpus": 1_000_000_000,
        "pids_limit": 64,
        "max_upload_bytes": 64 * 1024 * 1024,
        "max_output_bytes": 8 * 1024 * 1024,
    }))
    .unwrap();
    assert_eq!(
        settings.platform, "linux/amd64",
        "the real-Vyper Docker test uses the Linux amd64 release manifest"
    );

    let compilers_dir = tempfile::tempdir().unwrap();
    let fetcher = ListFetcher::<DetailedVersion>::new(
        Url::parse(LINUX_AMD64_VYPER_COMPILER_LIST).unwrap(),
        compilers_dir.path().to_path_buf(),
        None,
        None,
    )
    .await
    .unwrap();
    let version = DetailedVersion::from_str("0.4.3+commit.bff19ea2").unwrap();
    let compiler = fetcher.fetch(&version).await.unwrap();

    let executor = DockerCompilerExecutor::connect(settings).await.unwrap();
    let input = serde_json::to_vec(&serde_json::json!({
        "language": "Vyper",
        "sources": {
            "source.vy": {
                "content": "@external\ndef answer() -> uint256:\n    return 42\n"
            }
        },
        "settings": {
            "outputSelection": {
                "source.vy": ["abi", "evm.bytecode"]
            }
        }
    }))
    .unwrap();
    let invocation = CompilerInvocation::new(
        JobFile::executable("compiler", "bin/vyper", compiler).unwrap(),
        vec![CommandArgument::literal("--standard-json")],
        Bytes::from(input),
    );

    for _ in 0..2 {
        let output = executor.execute(invocation.clone()).await.unwrap();
        output.ensure_success("vyper").unwrap();
        let output: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            output.pointer("/contracts/source.vy/source").is_some(),
            "unexpected Vyper output: {output}"
        );
    }
}
