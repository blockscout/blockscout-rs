// SPDX-License-Identifier: LicenseRef-Blockscout

//! Verifies contracts compiled by [Solar](https://github.com/paradigmxyz/solar) through the
//! Solidity verifier, running the real compiler from the Solar compiler list.
//! The fixtures were produced by Solar v0.3.0 from `test_cases_solar/*.json`'s `input`.
//!
//! The compiler runs on the host by default. With `SCV_TEST_DOCKER_ADDR` and
//! `SCV_TEST_DOCKER_RUNNER_IMAGE` set (and optionally `SCV_TEST_DOCKER_KEY_PATH`), it runs
//! through the Docker executor instead, as in production, using the linux/amd64 list.
//! `SCV_TEST_SOLAR_LIST_URL` replaces the compiler list, e.g. with one that is not published yet.

use actix_web::{
    test,
    test::{read_body_json, TestRequest},
    App,
};
use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde_json::json;
use smart_contract_verifier::{
    CompilerExecutor, ConcurrencyLimitedCompilerExecutor, DockerCompilerExecutor,
    DockerCompilerExecutorSettings, NativeCompilerExecutor,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    solidity_verifier_actix::route_solidity_verifier,
    source::{MatchType, SourceType},
    verify_response::Status,
    ListCompilerVersionsResponse, VerifyResponse,
};
use smart_contract_verifier_server::{Settings, SolidityVerifierService};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::OnceCell;

const STANDARD_JSON_ROUTE: &str = "/api/v2/verifier/solidity/sources:verify-standard-json";
const MULTI_PART_ROUTE: &str = "/api/v2/verifier/solidity/sources:verify-multi-part";
const VERSIONS_ROUTE: &str = "/api/v2/verifier/solidity/versions";
/// Solar v0.3.0, as the Solidity verifier lists it.
const COMPILER_VERSION: &str = "v0.8.38+commit.e7a91fc7.solar.0.3.0";
/// The same compiler as its version is written in contract metadata and as
/// `solar --version` prints it.
const COMPILER_VERSION_ALIASES: [&str; 2] = [
    "0.8.38+commit.e7a91fc7.solar.0.3.0",
    "v0.3.0+commit.e7a91fc7",
];

/// The list the Docker executor needs whatever the host is: it runs linux/amd64 compilers.
const LINUX_AMD64_SOLAR_COMPILER_LIST: &str =
    "https://raw.githubusercontent.com/blockscout/solc-bin/main/solar.linux-amd64.list.json";

/// Docker executor settings from the environment, if the tests should run through Docker.
fn docker_settings() -> Option<DockerCompilerExecutorSettings> {
    let addr = std::env::var("SCV_TEST_DOCKER_ADDR").ok()?;
    let settings = serde_json::from_value(json!({
        "addr": addr,
        "key_path": std::env::var("SCV_TEST_DOCKER_KEY_PATH").ok(),
        "runner_image": std::env::var("SCV_TEST_DOCKER_RUNNER_IMAGE")
            .expect("SCV_TEST_DOCKER_RUNNER_IMAGE must be set with SCV_TEST_DOCKER_ADDR"),
    }))
    .expect("invalid Docker test settings");
    Some(settings)
}

/// The Docker executor keeps an SSH connection bound to the runtime that opened it, and every
/// `#[tokio::test]` has its own runtime, so only the native service is shared between tests.
/// Each service has its own compilers directory, which the caller keeps alive, so parallel
/// tests do not download into the same files.
async fn service() -> (Arc<SolidityVerifierService>, Option<TempDir>) {
    static NATIVE_SERVICE: OnceCell<(Arc<SolidityVerifierService>, TempDir)> =
        OnceCell::const_new();
    match docker_settings() {
        Some(docker) => {
            let executor = DockerCompilerExecutor::connect(docker)
                .await
                .expect("couldn't connect to the Docker host");
            let (service, compilers_dir) =
                new_service(Some(LINUX_AMD64_SOLAR_COMPILER_LIST), Arc::new(executor)).await;
            (service, Some(compilers_dir))
        }
        None => {
            let (service, _compilers_dir) = NATIVE_SERVICE
                .get_or_init(|| async {
                    // The default list is the one for the host platform.
                    new_service(None, Arc::new(NativeCompilerExecutor::default())).await
                })
                .await;
            (service.clone(), None)
        }
    }
}

async fn new_service(
    solar_list_url: Option<&str>,
    executor: Arc<dyn CompilerExecutor>,
) -> (Arc<SolidityVerifierService>, TempDir) {
    let compilers_dir = tempfile::tempdir().unwrap();
    let mut settings = Settings::default();
    settings.solidity.compilers_dir = compilers_dir.path().to_path_buf();
    settings.solidity.solar.enabled = true;
    let solar_list_url = std::env::var("SCV_TEST_SOLAR_LIST_URL")
        .ok()
        .or(solar_list_url.map(str::to_string));
    if let Some(list_url) = solar_list_url {
        settings.solidity.solar.fetcher =
            serde_json::from_value(json!({ "list": { "list_url": list_url } }))
                .expect("invalid Solar compiler list url");
    }
    let executor = Arc::new(ConcurrencyLimitedCompilerExecutor::new(
        executor,
        settings.compilers.max_threads,
    ));
    let service = SolidityVerifierService::new_with_admitted_executor(settings.solidity, executor)
        .await
        .expect("couldn't initialize the service");
    (Arc::new(service), compilers_dir)
}

#[derive(Deserialize)]
struct TestCase {
    input: serde_json::Value,
    child_creation: String,
    a_runtime: String,
    library: String,
}

fn test_case() -> TestCase {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/test_cases_solar/library_and_immutables.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

async fn verify(route: &str, request: serde_json::Value) -> VerifyResponse {
    let (service, _compilers_dir) = service().await;
    let app = test::init_service(
        App::new().configure(|config| route_solidity_verifier(config, service.clone())),
    )
    .await;
    let response = TestRequest::post()
        .uri(route)
        .set_json(&request)
        .send_request(&app)
        .await;
    let status = response.status();
    if !status.is_success() {
        let body = test::read_body(response).await;
        panic!(
            "invalid status code: {status}, body: {}",
            String::from_utf8_lossy(&body)
        );
    }
    read_body_json(response).await
}

#[tokio::test]
async fn lists_solar_versions_among_solidity_ones() {
    let (service, _compilers_dir) = service().await;
    let app = test::init_service(
        App::new().configure(|config| route_solidity_verifier(config, service.clone())),
    )
    .await;
    let response = TestRequest::get()
        .uri(VERSIONS_ROUTE)
        .send_request(&app)
        .await;
    assert!(response.status().is_success());
    let versions: ListCompilerVersionsResponse = read_body_json(response).await;
    let versions = versions.compiler_versions;

    // The Solidity part of the version is read from the Solar executable itself.
    assert!(
        versions.contains(&COMPILER_VERSION.to_string()),
        "{versions:?}"
    );
    assert!(
        versions.iter().any(|version| !version.contains(".solar.")),
        "{versions:?}"
    );
}

#[tokio::test]
async fn verifies_creation_code_with_constructor_arguments() {
    let test_case = test_case();
    for compiler_version in [COMPILER_VERSION]
        .into_iter()
        .chain(COMPILER_VERSION_ALIASES)
    {
        let response = verify(
            STANDARD_JSON_ROUTE,
            json!({
                "bytecode": test_case.child_creation,
                "bytecodeType": "CREATION_INPUT",
                "compilerVersion": compiler_version,
                "input": test_case.input.to_string(),
            }),
        )
        .await;
        assert_eq!(
            response.status(),
            Status::Success,
            "{compiler_version}: {response:?}"
        );

        let source = response.source.expect("source is missing");
        assert_eq!(source.file_name, "contracts/A.sol");
        assert_eq!(source.contract_name, "Child");
        assert_eq!(source.compiler_version, COMPILER_VERSION);
        assert_eq!(source.source_type(), SourceType::Solidity);
        assert_eq!(source.match_type(), MatchType::Full);
        assert_eq!(
            source.constructor_arguments.as_deref(),
            Some("0x000000000000000000000000000000000000000000000000000000000000002a")
        );

        // Solar reports `evm.bytecode.cborAuxdata` itself; ours is derived from a second
        // compilation and must agree with it (offset 91, the `solar` CBOR key, length suffix).
        let creation_artifacts: serde_json::Value =
            serde_json::from_str(&source.creation_input_artifacts.unwrap()).unwrap();
        assert_eq!(
            creation_artifacts["cborAuxdata"],
            json!({ "1": {
                "offset": 91,
                "value": "0xa264697066735822122034394bc99d77051429e3561ac1829ee821fba6c2c192224b8638c7049fcc6c1765736f6c6172430003000034",
            }})
        );
    }
}

#[tokio::test]
async fn verifies_runtime_code_with_library_and_immutables() {
    let test_case = test_case();
    let response = verify(
        STANDARD_JSON_ROUTE,
        json!({
            "bytecode": test_case.a_runtime,
            "bytecodeType": "DEPLOYED_BYTECODE",
            "compilerVersion": COMPILER_VERSION,
            "input": test_case.input.to_string(),
        }),
    )
    .await;
    assert_eq!(response.status(), Status::Success, "{response:?}");

    let source = response.source.expect("source is missing");
    assert_eq!(source.contract_name, "A");
    assert_eq!(source.match_type(), MatchType::Full);
    assert_eq!(
        source.libraries.get("contracts/A.sol:L"),
        Some(&test_case.library)
    );
}

#[tokio::test]
async fn reports_mismatch_as_failure() {
    let test_case = test_case();
    let mut bytecode = test_case.a_runtime;
    // Flip a byte in the code before the metadata, so it cannot match.
    bytecode.replace_range(10..12, "ff");
    let response = verify(
        STANDARD_JSON_ROUTE,
        json!({
            "bytecode": bytecode,
            "bytecodeType": "DEPLOYED_BYTECODE",
            "compilerVersion": COMPILER_VERSION,
            "input": test_case.input.to_string(),
        }),
    )
    .await;
    assert_eq!(response.status(), Status::Failure, "{response:?}");
}

#[tokio::test]
async fn rejects_multi_part_verification() {
    let test_case = test_case();
    for compiler_version in [COMPILER_VERSION]
        .into_iter()
        .chain(COMPILER_VERSION_ALIASES)
    {
        let response = verify(
            MULTI_PART_ROUTE,
            json!({
                "bytecode": test_case.a_runtime,
                "bytecodeType": "DEPLOYED_BYTECODE",
                "compilerVersion": compiler_version,
                "sourceFiles": { "A.sol": "contract A {}" },
            }),
        )
        .await;
        assert_eq!(
            response.status(),
            Status::Failure,
            "{compiler_version}: {response:?}"
        );
        assert!(
            response.message.contains("standard-json"),
            "{compiler_version}: {response:?}"
        );
    }
}
