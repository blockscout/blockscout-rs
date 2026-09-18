// SPDX-License-Identifier: LicenseRef-Blockscout

//! Settings tests that mutate the process environment. They live in their own test binary, where
//! every test holds `SETTINGS_ENVIRONMENT_LOCK`, so no other test reads the environment while one
//! of them changes it.

use blockscout_service_launcher::launcher::ConfigSettings;
use smart_contract_verifier::DockerCompilerExecutorSettings;
use smart_contract_verifier_server::{CompilerExecutionSettings, Settings};
use std::{ffi::OsString, sync::Mutex};

static SETTINGS_ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

const COMPILER_EXECUTION_ENVIRONMENT_KEYS: &[&str] = &[
    "SMART_CONTRACT_VERIFIER__CONFIG",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__TYPE",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__ADDR",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__KEY_PATH",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__RUNNER_IMAGE",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__PLATFORM",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__CONNECT_TIMEOUT_SECONDS",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__API_TIMEOUT_SECONDS",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__EXECUTION_TIMEOUT_SECONDS",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__MEMORY_LIMIT_BYTES",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__NANO_CPUS",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__PIDS_LIMIT",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__MAX_CONCURRENT_JOBS",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__MAX_UPLOAD_BYTES",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__MAX_OUTPUT_BYTES",
    "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__RUNTIME",
];

struct EnvironmentGuard {
    original: Vec<(String, Option<OsString>)>,
}

impl EnvironmentGuard {
    fn compiler_execution(overrides: &[(&str, &str)]) -> Self {
        let original = COMPILER_EXECUTION_ENVIRONMENT_KEYS
            .iter()
            .map(|key| ((*key).to_string(), std::env::var_os(key)))
            .collect();

        for key in COMPILER_EXECUTION_ENVIRONMENT_KEYS {
            std::env::remove_var(key);
        }
        for (key, value) in overrides {
            assert!(COMPILER_EXECUTION_ENVIRONMENT_KEYS.contains(key));
            std::env::set_var(key, value);
        }

        Self { original }
    }
}

impl Drop for EnvironmentGuard {
    fn drop(&mut self) {
        for (key, value) in self.original.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn docker(settings: &Settings) -> &DockerCompilerExecutorSettings {
    match &settings.compilers.execution {
        CompilerExecutionSettings::Docker(docker) => docker,
        other => panic!("expected Docker execution settings, got {other:?}"),
    }
}

#[test]
fn native_execution_builds_from_string_numeric_environment_values() {
    let _environment_lock = SETTINGS_ENVIRONMENT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = EnvironmentGuard::compiler_execution(&[
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__TYPE",
            "native",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__EXECUTION_TIMEOUT_SECONDS",
            "901",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__MAX_OUTPUT_BYTES",
            "902",
        ),
    ]);

    let settings = Settings::build().unwrap();
    assert!(matches!(
        settings.compilers.execution,
        CompilerExecutionSettings::Native {
            execution_timeout_seconds: 901,
            max_output_bytes: 902,
        }
    ));
}

#[test]
fn native_execution_builds_from_typed_numeric_toml_values() {
    let _environment_lock = SETTINGS_ENVIRONMENT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("settings.toml");
    std::fs::write(
        &config_path,
        r#"
[compilers.execution]
type = "native"
execution_timeout_seconds = 903
max_output_bytes = 904
"#,
    )
    .unwrap();
    let _environment = EnvironmentGuard::compiler_execution(&[(
        "SMART_CONTRACT_VERIFIER__CONFIG",
        config_path.to_str().unwrap(),
    )]);

    let settings = Settings::build().unwrap();
    assert!(matches!(
        settings.compilers.execution,
        CompilerExecutionSettings::Native {
            execution_timeout_seconds: 903,
            max_output_bytes: 904,
        }
    ));
}

#[test]
fn native_execution_rejects_malformed_numeric_environment_values() {
    let _environment_lock = SETTINGS_ENVIRONMENT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = EnvironmentGuard::compiler_execution(&[
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__TYPE",
            "native",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__MAX_OUTPUT_BYTES",
            "not-a-number",
        ),
    ]);

    let error = Settings::build().unwrap_err().to_string();
    assert!(error.contains("compilers.execution"), "{error}");
    assert!(error.contains("invalid digit"), "{error}");
}

#[test]
fn docker_execution_builds_from_string_numeric_environment_values() {
    let _environment_lock = SETTINGS_ENVIRONMENT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = EnvironmentGuard::compiler_execution(&[
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__TYPE",
            "docker",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__ADDR",
            "ssh://compiler-runner@example.org",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__RUNNER_IMAGE",
            "compiler-runner@sha256:0000000000000000000000000000000000000000000000000000000000000000",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__CONNECT_TIMEOUT_SECONDS",
            "7",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__API_TIMEOUT_SECONDS",
            "11",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__EXECUTION_TIMEOUT_SECONDS",
            "22",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__MEMORY_LIMIT_BYTES",
            "33",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__NANO_CPUS",
            "44",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__PIDS_LIMIT",
            "55",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__MAX_UPLOAD_BYTES",
            "77",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__MAX_OUTPUT_BYTES",
            "88",
        ),
    ]);

    let settings = Settings::build().unwrap();
    let docker = docker(&settings);

    assert_eq!(docker.connect_timeout_seconds, 7);
    assert_eq!(docker.api_timeout_seconds, 11);
    assert_eq!(docker.execution_timeout_seconds, 22);
    assert_eq!(docker.memory_limit_bytes, 33);
    assert_eq!(docker.nano_cpus, 44);
    assert_eq!(docker.pids_limit, 55);
    assert_eq!(docker.max_upload_bytes, 77);
    assert_eq!(docker.max_output_bytes, 88);
}

#[test]
fn docker_execution_rejects_malformed_numeric_environment_values() {
    let _environment_lock = SETTINGS_ENVIRONMENT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = EnvironmentGuard::compiler_execution(&[
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__TYPE",
            "docker",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__ADDR",
            "ssh://compiler-runner@example.org",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__RUNNER_IMAGE",
            "compiler-runner@sha256:0000000000000000000000000000000000000000000000000000000000000000",
        ),
        (
            "SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__CONNECT_TIMEOUT_SECONDS",
            "not-a-number",
        ),
    ]);

    let error = Settings::build().unwrap_err().to_string();

    assert!(error.contains("compilers.execution"), "{error}");
    assert!(error.contains("invalid digit"), "{error}");
}

#[test]
fn docker_execution_builds_from_typed_numeric_toml_values() {
    let _environment_lock = SETTINGS_ENVIRONMENT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("settings.toml");
    std::fs::write(
        &config_path,
        format!(
            r#"
[compilers.execution]
type = "docker"
addr = "ssh://compiler-runner@example.org"
runner_image = "compiler-runner@sha256:{}"
connect_timeout_seconds = 51
api_timeout_seconds = 101
execution_timeout_seconds = 202
memory_limit_bytes = 303
nano_cpus = 404
pids_limit = 505
max_upload_bytes = 707
max_output_bytes = 808
"#,
            "0".repeat(64)
        ),
    )
    .unwrap();
    let config_path = config_path.to_str().unwrap();
    let _environment =
        EnvironmentGuard::compiler_execution(&[("SMART_CONTRACT_VERIFIER__CONFIG", config_path)]);

    let settings = Settings::build().unwrap();
    let docker = docker(&settings);

    assert_eq!(docker.connect_timeout_seconds, 51);
    assert_eq!(docker.api_timeout_seconds, 101);
    assert_eq!(docker.execution_timeout_seconds, 202);
    assert_eq!(docker.memory_limit_bytes, 303);
    assert_eq!(docker.nano_cpus, 404);
    assert_eq!(docker.pids_limit, 505);
    assert_eq!(docker.max_upload_bytes, 707);
    assert_eq!(docker.max_output_bytes, 808);
}
