// SPDX-License-Identifier: LicenseRef-Blockscout

use anyhow::anyhow;
use blockscout_service_launcher::{
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use cron::Schedule;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr, PickFirst};
use smart_contract_verifier::{
    DockerCompilerExecutorSettings, DEFAULT_ERA_SOLIDITY_COMPILER_LIST,
    DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_SOURCIFY_HOST, DEFAULT_VYPER_COMPILER_LIST,
    DEFAULT_ZKSOLC_COMPILER_LIST,
};
use std::{
    num::{NonZeroU32, NonZeroUsize},
    path::PathBuf,
    str::FromStr,
};
use url::Url;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub server: ServerSettings,
    pub solidity: SoliditySettings,
    pub vyper: VyperSettings,
    pub sourcify: SourcifySettings,
    pub zksync_solidity: ZksyncSoliditySettings,
    pub metrics: MetricsSettings,
    pub jaeger: JaegerSettings,
    pub tracing: TracingSettings,
    pub compilers: CompilersSettings,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SoliditySettings {
    pub enabled: bool,
    pub compilers_dir: PathBuf,
    #[serde_as(as = "DisplayFromStr")]
    pub refresh_versions_schedule: Schedule,
    pub fetcher: FetcherSettings,
}

impl Default for SoliditySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            compilers_dir: default_compilers_dir("solidity-compilers"),
            refresh_versions_schedule: schedule_every_hour(),
            fetcher: default_list_fetcher(DEFAULT_SOLIDITY_COMPILER_LIST),
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VyperSettings {
    pub enabled: bool,
    pub compilers_dir: PathBuf,
    #[serde_as(as = "DisplayFromStr")]
    pub refresh_versions_schedule: Schedule,
    pub fetcher: FetcherSettings,
}

impl Default for VyperSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            compilers_dir: default_compilers_dir("vyper-compilers"),
            refresh_versions_schedule: schedule_every_hour(),
            fetcher: default_list_fetcher(DEFAULT_VYPER_COMPILER_LIST),
        }
    }
}

#[derive(Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum FetcherSettings {
    List(ListFetcherSettings),
    S3(S3FetcherSettings),
}

#[derive(Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ListFetcherSettings {
    pub list_url: Url,
}

#[derive(Deserialize, Default, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct S3FetcherSettings {
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    pub bucket: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SourcifySettings {
    pub enabled: bool,
    pub api_url: Url,
    /// Number of attempts the server makes to Sourcify API.
    /// Should be at least one. Set to `3` by default.
    pub verification_attempts: NonZeroU32,
    pub request_timeout: u64,
    /// Interval, in milliseconds, between polls of an asynchronous Sourcify
    /// (API v2) verification job. Set to `1000` by default.
    pub poll_interval_ms: u64,
    /// Maximum number of times an asynchronous Sourcify (API v2) verification
    /// job is polled before giving up. Combined with `poll_interval_ms` this
    /// bounds the total time spent waiting for a verification to complete.
    /// Set to `120` by default (~120 seconds).
    pub max_poll_attempts: NonZeroU32,
}

impl Default for SourcifySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            api_url: Url::try_from(DEFAULT_SOURCIFY_HOST).expect("valid url"),
            verification_attempts: NonZeroU32::new(3).expect("Is not zero"),
            request_timeout: 15,
            poll_interval_ms: 1000,
            max_poll_attempts: NonZeroU32::new(120).expect("Is not zero"),
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZksyncSoliditySettings {
    pub enabled: bool,
    pub evm_compilers_dir: PathBuf,
    #[serde_as(as = "DisplayFromStr")]
    pub evm_refresh_versions_schedule: Schedule,
    pub evm_fetcher: FetcherSettings,
    pub era_evm_fetcher: FetcherSettings,
    pub zk_compilers_dir: PathBuf,
    #[serde_as(as = "DisplayFromStr")]
    pub zk_refresh_versions_schedule: Schedule,
    pub zk_fetcher: FetcherSettings,
}

impl Default for ZksyncSoliditySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            evm_compilers_dir: default_compilers_dir("zksync-solc-compilers"),
            evm_refresh_versions_schedule: schedule_every_hour(),
            evm_fetcher: default_list_fetcher(DEFAULT_SOLIDITY_COMPILER_LIST),
            era_evm_fetcher: default_list_fetcher(DEFAULT_ERA_SOLIDITY_COMPILER_LIST),
            zk_compilers_dir: default_compilers_dir("zksync-zksolc-compilers"),
            zk_refresh_versions_schedule: schedule_every_hour(),
            zk_fetcher: default_list_fetcher(DEFAULT_ZKSOLC_COMPILER_LIST),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CompilersSettings {
    pub max_threads: NonZeroUsize,
    pub execution: CompilerExecutionSettings,
}

impl Default for CompilersSettings {
    fn default() -> Self {
        let max_threads = std::thread::available_parallelism().unwrap_or_else(|e| {
            tracing::warn!("cannot get number of CPU cores: {}", e);
            NonZeroUsize::new(8).unwrap()
        });
        Self {
            max_threads,
            execution: CompilerExecutionSettings::Disabled,
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompilerExecutionSettings {
    /// Compiler-backed endpoints cannot start in this mode.
    Disabled,
    /// Explicit development/test escape hatch. Compilers execute on the service host.
    Native {
        #[serde(default = "default_compiler_execution_timeout_seconds")]
        #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
        execution_timeout_seconds: u64,
        #[serde(default = "default_compiler_max_output_bytes")]
        #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
        max_output_bytes: usize,
    },
    /// One fresh, locked-down container per compiler invocation on an SSH Docker host.
    Docker {
        addr: String,
        key_path: Option<PathBuf>,
        runner_image: String,
        #[serde(default = "default_compiler_platform")]
        platform: String,
        #[serde(default = "default_docker_connect_timeout_seconds")]
        #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
        connect_timeout_seconds: u64,
        #[serde(default = "default_docker_api_timeout_seconds")]
        #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
        api_timeout_seconds: u64,
        #[serde(default = "default_compiler_execution_timeout_seconds")]
        #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
        execution_timeout_seconds: u64,
        #[serde(default = "default_compiler_memory_limit_bytes")]
        #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
        memory_limit_bytes: i64,
        #[serde(default = "default_compiler_nano_cpus")]
        #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
        nano_cpus: i64,
        #[serde(default = "default_compiler_pids_limit")]
        #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
        pids_limit: i64,
        #[serde(default = "default_compiler_max_upload_bytes")]
        #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
        max_upload_bytes: usize,
        #[serde(default = "default_compiler_max_output_bytes")]
        #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
        max_output_bytes: usize,
        runtime: Option<String>,
    },
}

impl CompilerExecutionSettings {
    pub(crate) fn docker_executor_settings(&self) -> Option<DockerCompilerExecutorSettings> {
        match self {
            Self::Docker {
                addr,
                key_path,
                runner_image,
                platform,
                connect_timeout_seconds,
                api_timeout_seconds,
                execution_timeout_seconds,
                memory_limit_bytes,
                nano_cpus,
                pids_limit,
                max_upload_bytes,
                max_output_bytes,
                runtime,
            } => Some(DockerCompilerExecutorSettings {
                addr: addr.clone(),
                key_path: key_path.clone(),
                runner_image: runner_image.clone(),
                platform: platform.clone(),
                connect_timeout_seconds: *connect_timeout_seconds,
                api_timeout_seconds: *api_timeout_seconds,
                execution_timeout_seconds: *execution_timeout_seconds,
                memory_limit_bytes: *memory_limit_bytes,
                nano_cpus: *nano_cpus,
                pids_limit: *pids_limit,
                max_upload_bytes: *max_upload_bytes,
                max_output_bytes: *max_output_bytes,
                runtime: runtime.clone(),
            }),
            Self::Disabled | Self::Native { .. } => None,
        }
    }
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "SMART_CONTRACT_VERIFIER";

    fn validate(&self) -> anyhow::Result<()> {
        // Validate s3 fetcher
        if let FetcherSettings::S3(settings) = &self.solidity.fetcher {
            if settings.region.is_none() && settings.endpoint.is_none() {
                return Err(anyhow!("for s3 fetcher settings at least one of `region` or `endpoint` should be defined"));
            }
        };

        let compiler_endpoints_enabled =
            self.solidity.enabled || self.vyper.enabled || self.zksync_solidity.enabled;
        match &self.compilers.execution {
            CompilerExecutionSettings::Disabled if compiler_endpoints_enabled => {
                return Err(anyhow!(
                    "compiler execution is disabled while a compiler-backed endpoint is enabled"
                ));
            }
            CompilerExecutionSettings::Docker { .. } => self
                .compilers
                .execution
                .docker_executor_settings()
                .expect("docker settings must exist for Docker mode")
                .validate()
                .map_err(anyhow::Error::new)?,
            CompilerExecutionSettings::Native {
                execution_timeout_seconds,
                max_output_bytes,
            } if *execution_timeout_seconds == 0 || *max_output_bytes == 0 => {
                return Err(anyhow!(
                    "native compiler timeout and output limit must be positive"
                ));
            }
            CompilerExecutionSettings::Disabled | CompilerExecutionSettings::Native { .. } => {}
        }

        Ok(())
    }
}

fn default_compilers_dir<P: AsRef<std::path::Path>>(path: P) -> PathBuf {
    let mut compilers_dir = std::env::temp_dir();
    compilers_dir.push(path);
    compilers_dir
}

fn default_list_fetcher(list_url: &str) -> FetcherSettings {
    FetcherSettings::List(ListFetcherSettings {
        list_url: Url::try_from(list_url).expect("invalid default list.json url"),
    })
}

fn schedule_every_hour() -> Schedule {
    Schedule::from_str("0 0 * * * * *").unwrap()
}

fn default_compiler_platform() -> String {
    "linux/amd64".to_string()
}

fn default_docker_api_timeout_seconds() -> u64 {
    30
}

fn default_docker_connect_timeout_seconds() -> u64 {
    30
}

fn default_compiler_execution_timeout_seconds() -> u64 {
    120
}

fn default_compiler_memory_limit_bytes() -> i64 {
    1024 * 1024 * 1024
}

fn default_compiler_nano_cpus() -> i64 {
    2_000_000_000
}

fn default_compiler_pids_limit() -> i64 {
    64
}

fn default_compiler_max_upload_bytes() -> usize {
    256 * 1024 * 1024
}

fn default_compiler_max_output_bytes() -> usize {
    32 * 1024 * 1024
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn compiler_endpoints_fail_closed_by_default() {
        let settings = Settings::default();
        assert!(ConfigSettings::validate(&settings).is_err());
    }

    #[test]
    fn disabled_execution_is_allowed_without_compiler_endpoints() {
        let mut settings = Settings::default();
        settings.solidity.enabled = false;
        settings.vyper.enabled = false;
        settings.zksync_solidity.enabled = false;
        ConfigSettings::validate(&settings).unwrap();
    }

    #[test]
    fn native_execution_must_be_explicit() {
        let mut settings = Settings::default();
        settings.compilers.execution = CompilerExecutionSettings::Native {
            execution_timeout_seconds: default_compiler_execution_timeout_seconds(),
            max_output_bytes: default_compiler_max_output_bytes(),
        };
        ConfigSettings::validate(&settings).unwrap();
    }

    #[test]
    fn native_execution_deserializes_with_defaults() {
        let settings: Settings = serde_json::from_value(serde_json::json!({
            "compilers": {
                "execution": {
                    "type": "native"
                }
            }
        }))
        .unwrap();

        assert!(matches!(
            settings.compilers.execution,
            CompilerExecutionSettings::Native {
                execution_timeout_seconds: 120,
                max_output_bytes: 33_554_432,
            }
        ));
        ConfigSettings::validate(&settings).unwrap();
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
    fn native_execution_rejects_zero_limits() {
        let mut settings = Settings::default();
        settings.compilers.execution = CompilerExecutionSettings::Native {
            execution_timeout_seconds: 0,
            max_output_bytes: 1,
        };
        assert!(ConfigSettings::validate(&settings).is_err());

        settings.compilers.execution = CompilerExecutionSettings::Native {
            execution_timeout_seconds: 1,
            max_output_bytes: 0,
        };
        assert!(ConfigSettings::validate(&settings).is_err());
    }

    #[test]
    fn docker_execution_rejects_unpinned_images() {
        let mut settings = Settings::default();
        settings.compilers.execution = CompilerExecutionSettings::Docker {
            addr: "ssh://compiler-runner@example.org".to_string(),
            key_path: None,
            runner_image: "compiler-runner:latest".to_string(),
            platform: default_compiler_platform(),
            connect_timeout_seconds: default_docker_connect_timeout_seconds(),
            api_timeout_seconds: default_docker_api_timeout_seconds(),
            execution_timeout_seconds: default_compiler_execution_timeout_seconds(),
            memory_limit_bytes: default_compiler_memory_limit_bytes(),
            nano_cpus: default_compiler_nano_cpus(),
            pids_limit: default_compiler_pids_limit(),
            max_upload_bytes: default_compiler_max_upload_bytes(),
            max_output_bytes: default_compiler_max_output_bytes(),
            runtime: None,
        };
        assert!(ConfigSettings::validate(&settings).is_err());
    }

    #[test]
    fn docker_execution_deserializes_with_safe_defaults() {
        let settings: Settings = serde_json::from_value(serde_json::json!({
            "compilers": {
                "execution": {
                    "type": "docker",
                    "addr": "ssh://compiler-runner@example.org",
                    "runner_image": format!("compiler-runner@sha256:{}", "0".repeat(64))
                }
            }
        }))
        .unwrap();

        let docker = settings
            .compilers
            .execution
            .docker_executor_settings()
            .unwrap();
        assert_eq!(docker.platform, "linux/amd64");
        assert_eq!(docker.connect_timeout_seconds, 30);
        assert_eq!(docker.api_timeout_seconds, 30);
        ConfigSettings::validate(&settings).unwrap();
    }

    #[test]
    fn docker_execution_rejects_the_removed_second_concurrency_limit() {
        let error = serde_json::from_value::<Settings>(serde_json::json!({
            "compilers": {
                "execution": {
                    "type": "docker",
                    "addr": "ssh://compiler-runner@example.org",
                    "runner_image": format!("compiler-runner@sha256:{}", "0".repeat(64)),
                    "max_concurrent_jobs": 8
                }
            }
        }))
        .unwrap_err()
        .to_string();

        assert!(error.contains("max_concurrent_jobs"), "{error}");
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
        let docker = settings
            .compilers
            .execution
            .docker_executor_settings()
            .unwrap();

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
        let _environment = EnvironmentGuard::compiler_execution(&[(
            "SMART_CONTRACT_VERIFIER__CONFIG",
            config_path,
        )]);

        let settings = Settings::build().unwrap();
        let docker = settings
            .compilers
            .execution
            .docker_executor_settings()
            .unwrap();

        assert_eq!(docker.connect_timeout_seconds, 51);
        assert_eq!(docker.api_timeout_seconds, 101);
        assert_eq!(docker.execution_timeout_seconds, 202);
        assert_eq!(docker.memory_limit_bytes, 303);
        assert_eq!(docker.nano_cpus, 404);
        assert_eq!(docker.pids_limit, 505);
        assert_eq!(docker.max_upload_bytes, 707);
        assert_eq!(docker.max_output_bytes, 808);
    }
}
