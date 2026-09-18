use super::{
    deliver_stdin,
    metrics::{self, ContainerFamily},
    CompilerExecutor, CompilerInvocation, ExecutionError, ExecutionOutput, JobFileContent,
};
use anyhow::Context;
use async_trait::async_trait;
use bollard::{
    container::LogOutput,
    models::{
        ContainerCreateBody, ContainerInspectResponse, ContainerStateStatusEnum, HostConfig,
        HostConfigLogConfig, Mount, MountType, MountVolumeOptions, MountVolumeOptionsDriverConfig,
        ResourcesUlimits, Volume, VolumeCreateRequest,
    },
    query_parameters::{
        AttachContainerOptionsBuilder, CreateContainerOptionsBuilder, ListContainersOptionsBuilder,
        RemoveContainerOptionsBuilder, UploadToContainerOptionsBuilder,
        WaitContainerOptionsBuilder,
    },
    Docker, API_DEFAULT_VERSION,
};
use bytes::Bytes;
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    io::{Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::{Mutex, OnceCell, OwnedSemaphorePermit},
    time::{sleep, timeout, Instant},
};
use tokio_util::io::ReaderStream;
use url::Url;
use uuid::Uuid;

const JOB_ROOT: &str = "/job";
const COMPILER_CACHE_ROOT: &str = "/compiler-cache";
const COMPILER_CACHE_MOUNT: &str = "/cache";
const COMPILER_CACHE_FILE: &str = "compiler";
const COMPILER_CACHE_VOLUME_PREFIX: &str = "scv-compiler-v1-sha256-";
const COMPILER_CACHE_INIT_PREFIX: &str = "scv-compiler-cache-init-v1-";
const COMPILER_CACHE_LABEL: &str = "org.blockscout.smart-contract-verifier.compiler-cache";
const COMPILER_CACHE_SCHEMA_LABEL: &str =
    "org.blockscout.smart-contract-verifier.compiler-cache-schema";
const COMPILER_CACHE_DIGEST_LABEL: &str = "org.blockscout.smart-contract-verifier.compiler-digest";
const COMPILER_CACHE_GENERATION_LABEL: &str =
    "org.blockscout.smart-contract-verifier.compiler-cache-generation";
const JOB_LABEL: &str = "org.blockscout.smart-contract-verifier.compiler-job";
const JOB_EXPIRY_LABEL: &str = "org.blockscout.smart-contract-verifier.expires-at";
const CONTAINER_FAMILY_LABEL: &str =
    "org.blockscout.smart-contract-verifier.compiler-container-family";
const JOB_USER: &str = "65532:65532";
const HOME_DIR: &str = "/tmp";
const COMPILER_TMP_DIR: &str = "/compiler-tmp";
const HOME_TMPFS_SIZE_BYTES: i64 = 64 * 1024 * 1024;
const COMPILER_TMPFS_SIZE_BYTES: i64 = 256 * 1024 * 1024;
const CLEANUP_GRACE_SECONDS: u64 = 300;
const JANITOR_INTERVAL_SECONDS: u64 = 30;

#[derive(Clone, Debug)]
pub struct DockerCompilerExecutorSettings {
    pub addr: String,
    pub key_path: Option<PathBuf>,
    pub runner_image: String,
    pub platform: String,
    pub connect_timeout_seconds: u64,
    pub api_timeout_seconds: u64,
    pub execution_timeout_seconds: u64,
    pub memory_limit_bytes: i64,
    pub nano_cpus: i64,
    pub pids_limit: i64,
    pub max_upload_bytes: usize,
    pub max_output_bytes: usize,
    pub runtime: Option<String>,
}

impl DockerCompilerExecutorSettings {
    pub fn validate(&self) -> Result<(), ExecutionError> {
        validate_settings(self)
    }

    fn request_timeout_seconds(&self) -> u64 {
        self.api_timeout_seconds.max(self.execution_timeout_seconds)
    }

    fn container_lifecycle_timeout_seconds(&self) -> u64 {
        self.execution_timeout_seconds
            .saturating_add(self.request_timeout_seconds().saturating_mul(2))
    }

    /// Budget for validating and seeding all compilers of one job, including waits for other
    /// in-process fills of the same digest.
    fn cache_phase_timeout_seconds(&self, compiler_count: usize) -> u64 {
        let compiler_count = u64::try_from(compiler_count).unwrap_or(u64::MAX);
        self.container_lifecycle_timeout_seconds()
            .saturating_mul(compiler_count)
    }

    /// The job container is created before its cache phase and must outlive it plus one run.
    fn job_container_lifecycle_timeout_seconds(&self, compiler_count: usize) -> u64 {
        self.cache_phase_timeout_seconds(compiler_count)
            .saturating_add(self.container_lifecycle_timeout_seconds())
    }
}

#[derive(Clone)]
pub struct DockerCompilerExecutor {
    docker: Docker,
    settings: Arc<DockerCompilerExecutorSettings>,
    compiler_cache: Arc<CompilerCacheState>,
    initialization: Arc<OnceCell<Arc<ContainerJanitor>>>,
}

#[derive(Default)]
struct CompilerCacheState {
    ready: Mutex<HashMap<String, String>>,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    local_digests: LocalDigestCache,
}

/// Identity of a local compiler file. `ctime` changes on every content or metadata change and
/// cannot be set from user space, so an unchanged fingerprint keeps a previously computed digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalFileFingerprint {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}

impl LocalFileFingerprint {
    fn new(metadata: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            len: metadata.len(),
            mtime: (metadata.mtime(), metadata.mtime_nsec()),
            ctime: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }
}

/// Avoids rehashing unchanged compiler binaries on every warm-cache job.
#[derive(Default)]
struct LocalDigestCache(parking_lot::Mutex<HashMap<PathBuf, (LocalFileFingerprint, String)>>);

impl LocalDigestCache {
    fn digest(&self, path: &Path, metadata: &std::fs::Metadata) -> Result<String, ExecutionError> {
        let fingerprint = LocalFileFingerprint::new(metadata);
        if let Some((cached, digest)) = self.0.lock().get(path) {
            if *cached == fingerprint {
                return Ok(digest.clone());
            }
        }

        let mut source = std::fs::File::open(path)
            .with_context(|| format!("open compiler executable {}", path.display()))
            .map_err(ExecutionError::Infrastructure)?;
        let mut hasher = Sha256::new();
        std::io::copy(
            &mut ExactSizeReader::new(&mut source, fingerprint.len),
            &mut hasher,
        )
        .with_context(|| format!("hash compiler executable {}", path.display()))
        .map_err(ExecutionError::Infrastructure)?;
        let hashed = source
            .metadata()
            .with_context(|| format!("inspect compiler executable {}", path.display()))
            .map_err(ExecutionError::Infrastructure)?;
        if LocalFileFingerprint::new(&hashed) != fingerprint {
            return Err(ExecutionError::Infrastructure(anyhow::anyhow!(
                "compiler executable {} changed while it was being hashed",
                path.display()
            )));
        }

        let digest = hex::encode(hasher.finalize());
        self.0
            .lock()
            .insert(path.to_path_buf(), (fingerprint, digest.clone()));
        Ok(digest)
    }
}

#[derive(Debug)]
struct PreparedCompilers {
    /// Local source of each distinct compiler; read again only when a cache volume needs seeding.
    by_digest: BTreeMap<String, PathBuf>,
    path_overrides: HashMap<String, PathBuf>,
    logical_bytes: usize,
}

struct SpooledArchive {
    file: std::fs::File,
    len: usize,
}

impl SpooledArchive {
    fn len(&self) -> usize {
        self.len
    }

    fn into_stream(self) -> ReaderStream<tokio::fs::File> {
        ReaderStream::with_capacity(tokio::fs::File::from_std(self.file), 64 * 1024)
    }
}

impl DockerCompilerExecutor {
    /// Builds an executor without contacting the Docker host.
    ///
    /// Static configuration is still validated immediately. Remote initialization is performed by
    /// the first health check or compiler invocation and retried after transient failures.
    pub fn new(settings: DockerCompilerExecutorSettings) -> Result<Self, ExecutionError> {
        settings.validate()?;
        let key_path = match settings.key_path.as_deref() {
            Some(path) => Some(
                path.to_str()
                    .ok_or_else(|| {
                        ExecutionError::InvalidInvocation(
                            "docker SSH key path must contain valid UTF-8".to_string(),
                        )
                    })?
                    .to_string(),
            ),
            None => None,
        };

        // Large cold-cache transfers must not inherit the shorter control-plane timeout.
        let docker = Docker::connect_with_ssh(
            &settings.addr,
            settings.request_timeout_seconds(),
            API_DEFAULT_VERSION,
            key_path,
        )
        .map_err(|error| ExecutionError::Infrastructure(error.into()))?;

        Ok(Self {
            docker,
            settings: Arc::new(settings),
            compiler_cache: Arc::new(CompilerCacheState::default()),
            initialization: Arc::new(OnceCell::new()),
        })
    }

    /// Builds an executor and eagerly verifies the remote host for compatibility with direct users.
    pub async fn connect(settings: DockerCompilerExecutorSettings) -> Result<Self, ExecutionError> {
        let executor = Self::new(settings)?;
        executor.ensure_initialized().await?;
        Ok(executor)
    }

    async fn ensure_initialized(&self) -> Result<(), ExecutionError> {
        self.initialization
            .get_or_try_init(|| async {
                self.initialize_remote().await?;
                Ok(Arc::new(ContainerJanitor::spawn(self.docker.clone())))
            })
            .await
            .map(|_| ())
    }

    async fn initialize_remote(&self) -> Result<(), ExecutionError> {
        let mut ssh_builder = openssh::SessionBuilder::default();
        ssh_builder
            .known_hosts_check(openssh::KnownHosts::Strict)
            .connect_timeout(Duration::from_secs(self.settings.connect_timeout_seconds));
        if let Some(key_path) = self.settings.key_path.as_deref() {
            ssh_builder.keyfile(key_path);
        }
        let ssh_session = ssh_builder
            .connect(&self.settings.addr)
            .await
            .context("verify pinned Docker SSH host key")
            .map_err(ExecutionError::Infrastructure)?;
        ssh_session
            .close()
            .await
            .context("close Docker SSH host-key verification session")
            .map_err(ExecutionError::Infrastructure)?;

        self.docker
            .ping()
            .await
            .context("ping remote Docker daemon")
            .map_err(ExecutionError::Infrastructure)?;
        reap_expired_containers(&self.docker, "initialization")
            .await
            .context("reap expired compiler containers")
            .map_err(ExecutionError::Infrastructure)?;
        self.docker
            .inspect_image(&self.settings.runner_image)
            .await
            .with_context(|| {
                format!(
                    "compiler runner image {} is not present on the remote Docker host",
                    self.settings.runner_image
                )
            })
            .map_err(ExecutionError::Infrastructure)?;
        Ok(())
    }

    async fn check_remote_health(&self) -> Result<(), ExecutionError> {
        self.docker
            .ping()
            .await
            .context("ping remote Docker daemon")
            .map_err(ExecutionError::Infrastructure)?;
        self.docker
            .inspect_image(&self.settings.runner_image)
            .await
            .with_context(|| {
                format!(
                    "compiler runner image {} is not present on the remote Docker host",
                    self.settings.runner_image
                )
            })
            .map_err(ExecutionError::Infrastructure)?;
        Ok(())
    }

    fn container_config(
        &self,
        invocation: &CompilerInvocation,
        compilers: &PreparedCompilers,
    ) -> Result<ContainerCreateBody, ExecutionError> {
        let program = invocation
            .program_path_with_overrides(Path::new(JOB_ROOT), &compilers.path_overrides)?;
        let args = invocation
            .resolved_args_with_overrides(Path::new(JOB_ROOT), &compilers.path_overrides)?;
        let mut command = Vec::with_capacity(args.len() + 1);
        command.push(program.to_string_lossy().into_owned());
        command.extend(args);

        let now = unix_timestamp()?;
        let expiry = now
            .saturating_add(
                self.settings
                    .job_container_lifecycle_timeout_seconds(compilers.by_digest.len()),
            )
            .saturating_add(CLEANUP_GRACE_SECONDS);
        let labels = HashMap::from([
            (JOB_LABEL.to_string(), "true".to_string()),
            (JOB_EXPIRY_LABEL.to_string(), expiry.to_string()),
            (
                CONTAINER_FAMILY_LABEL.to_string(),
                ContainerFamily::Job.as_str().to_string(),
            ),
        ]);
        let tmpfs = HashMap::from([
            (
                HOME_DIR.to_string(),
                format!("rw,noexec,nosuid,nodev,size={HOME_TMPFS_SIZE_BYTES}"),
            ),
            (
                COMPILER_TMP_DIR.to_string(),
                format!(
                    "rw,exec,nosuid,nodev,size={COMPILER_TMPFS_SIZE_BYTES},uid=65532,gid=65532,mode=0700"
                ),
            ),
        ]);
        let runtime = self
            .settings
            .runtime
            .as_ref()
            .filter(|runtime| !runtime.is_empty())
            .cloned();

        let mut mounts = vec![Mount {
            target: Some(JOB_ROOT.to_string()),
            typ: Some(MountType::VOLUME),
            read_only: Some(false),
            volume_options: Some(MountVolumeOptions {
                no_copy: Some(true),
                labels: Some(labels.clone()),
                ..Default::default()
            }),
            ..Default::default()
        }];
        mounts.extend(compilers.by_digest.keys().map(|digest| {
            Mount {
                target: Some(
                    compiler_cache_mount_path(digest)
                        .parent()
                        .expect("compiler cache file always has a parent")
                        .to_string_lossy()
                        .into_owned(),
                ),
                source: Some(compiler_cache_volume_name(digest)),
                typ: Some(MountType::VOLUME),
                read_only: Some(true),
                volume_options: Some(compiler_cache_mount_options(digest)),
                ..Default::default()
            }
        }));

        let host_config = HostConfig {
            memory: Some(self.settings.memory_limit_bytes),
            memory_swap: Some(self.settings.memory_limit_bytes),
            nano_cpus: Some(self.settings.nano_cpus),
            oom_kill_disable: Some(false),
            init: Some(true),
            pids_limit: Some(self.settings.pids_limit),
            ulimits: Some(vec![ResourcesUlimits {
                name: Some("nofile".to_string()),
                soft: Some(1024),
                hard: Some(1024),
            }]),
            log_config: Some(HostConfigLogConfig {
                typ: Some("none".to_string()),
                config: Some(HashMap::new()),
            }),
            network_mode: Some("none".to_string()),
            auto_remove: Some(false),
            cap_drop: Some(vec!["ALL".to_string()]),
            privileged: Some(false),
            readonly_rootfs: Some(true),
            security_opt: Some(vec!["no-new-privileges:true".to_string()]),
            tmpfs: Some(tmpfs),
            mounts: Some(mounts),
            runtime,
            ..Default::default()
        };

        Ok(ContainerCreateBody {
            user: Some(JOB_USER.to_string()),
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(false),
            open_stdin: Some(true),
            stdin_once: Some(true),
            env: Some(vec![
                format!("HOME={HOME_DIR}"),
                format!("TMPDIR={COMPILER_TMP_DIR}"),
            ]),
            cmd: Some(command),
            image: Some(self.settings.runner_image.clone()),
            working_dir: Some(JOB_ROOT.to_string()),
            entrypoint: Some(vec![String::new()]),
            network_disabled: Some(true),
            labels: Some(labels),
            host_config: Some(host_config),
            ..Default::default()
        })
    }

    fn cache_helper_config(
        &self,
        digest: &str,
        initialize: bool,
    ) -> Result<ContainerCreateBody, ExecutionError> {
        let expiry = unix_timestamp()?
            .saturating_add(self.settings.container_lifecycle_timeout_seconds())
            .saturating_add(CLEANUP_GRACE_SECONDS);
        let labels = HashMap::from([
            (JOB_LABEL.to_string(), "true".to_string()),
            (JOB_EXPIRY_LABEL.to_string(), expiry.to_string()),
            (
                CONTAINER_FAMILY_LABEL.to_string(),
                if initialize {
                    ContainerFamily::CacheInitializer
                } else {
                    ContainerFamily::CacheProbe
                }
                .as_str()
                .to_string(),
            ),
        ]);
        let command = vec![
            "/bin/sh".to_string(),
            "-ceu".to_string(),
            if initialize {
                concat!(
                    "actual=$(/usr/bin/sha256sum /cache/.incoming); ",
                    "actual=${actual%% *}; ",
                    "[ \"$actual\" = \"$EXPECTED_DIGEST\" ]; ",
                    "/bin/chmod 0555 /cache/.incoming; ",
                    "/bin/mv -f /cache/.incoming /cache/compiler"
                )
            } else {
                concat!(
                    "[ -x /cache/compiler ]; ",
                    "actual=$(/usr/bin/sha256sum /cache/compiler); ",
                    "actual=${actual%% *}; ",
                    "[ \"$actual\" = \"$EXPECTED_DIGEST\" ]"
                )
            }
            .to_string(),
        ];
        let env = Some(vec![format!("EXPECTED_DIGEST={digest}")]);
        let host_config = HostConfig {
            memory: Some(self.settings.memory_limit_bytes),
            memory_swap: Some(self.settings.memory_limit_bytes),
            nano_cpus: Some(self.settings.nano_cpus),
            oom_kill_disable: Some(false),
            init: Some(true),
            pids_limit: Some(self.settings.pids_limit),
            ulimits: Some(vec![ResourcesUlimits {
                name: Some("nofile".to_string()),
                soft: Some(128),
                hard: Some(128),
            }]),
            log_config: Some(HostConfigLogConfig {
                typ: Some("none".to_string()),
                config: Some(HashMap::new()),
            }),
            network_mode: Some("none".to_string()),
            auto_remove: Some(false),
            cap_drop: Some(vec!["ALL".to_string()]),
            privileged: Some(false),
            readonly_rootfs: Some(true),
            security_opt: Some(vec!["no-new-privileges:true".to_string()]),
            mounts: Some(vec![Mount {
                target: Some(COMPILER_CACHE_MOUNT.to_string()),
                source: Some(compiler_cache_volume_name(digest)),
                typ: Some(MountType::VOLUME),
                read_only: Some(!initialize),
                volume_options: Some(compiler_cache_mount_options(digest)),
                ..Default::default()
            }]),
            runtime: self
                .settings
                .runtime
                .as_ref()
                .filter(|runtime| !runtime.is_empty())
                .cloned(),
            ..Default::default()
        };

        Ok(ContainerCreateBody {
            // The initializer only renames a daemon-uploaded, root-owned file in a root-owned
            // volume. It still has no capabilities, network, or writable root filesystem.
            user: Some(if initialize { "0:0" } else { JOB_USER }.to_string()),
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(false),
            open_stdin: Some(true),
            stdin_once: Some(true),
            env,
            cmd: Some(command),
            image: Some(self.settings.runner_image.clone()),
            working_dir: Some(COMPILER_CACHE_MOUNT.to_string()),
            entrypoint: Some(vec![String::new()]),
            network_disabled: Some(true),
            labels: Some(labels),
            host_config: Some(host_config),
            ..Default::default()
        })
    }

    async fn ensure_compiler_cached(
        &self,
        digest: &str,
        source: &Path,
        admission_permit: Option<Arc<OwnedSemaphorePermit>>,
    ) -> Result<(), ExecutionError> {
        if self.cached_generation_is_ready(digest).await? {
            return Ok(());
        }
        let digest_lock = {
            let mut locks = self.compiler_cache.locks.lock().await;
            locks
                .entry(digest.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _digest_guard = digest_lock.lock().await;
        if self.cached_generation_is_ready(digest).await? {
            return Ok(());
        }

        let cache_timeout_seconds = self.settings.container_lifecycle_timeout_seconds();
        let deadline = Instant::now() + Duration::from_secs(cache_timeout_seconds);
        loop {
            if Instant::now() >= deadline {
                return Err(ExecutionError::Timeout {
                    seconds: cache_timeout_seconds,
                });
            }
            if let Some(volume) = self.inspect_cache_volume(digest).await? {
                let generation = validate_cache_volume(&volume, digest)?;
                if self
                    .probe_cached_compiler(digest, admission_permit.clone())
                    .await?
                {
                    self.compiler_cache
                        .ready
                        .lock()
                        .await
                        .insert(digest.to_string(), generation);
                    return Ok(());
                }
                self.compiler_cache.ready.lock().await.remove(digest);
            } else {
                self.compiler_cache.ready.lock().await.remove(digest);
                let volume = self
                    .docker
                    .create_volume(VolumeCreateRequest {
                        name: Some(compiler_cache_volume_name(digest)),
                        driver: Some("local".to_string()),
                        labels: Some(new_compiler_cache_labels(digest)),
                        ..Default::default()
                    })
                    .await
                    .context("create compiler cache volume")
                    .map_err(ExecutionError::Infrastructure)?;
                validate_cache_volume(&volume, digest)?;
            }

            match self
                .try_seed_compiler(digest, source.to_path_buf(), admission_permit.clone())
                .await?
            {
                SeedAttempt::Seeded => {}
                SeedAttempt::Busy => {
                    self.wait_for_cache_initializer(digest, deadline, cache_timeout_seconds)
                        .await?;
                }
            }
        }
    }

    async fn cached_generation_is_ready(&self, digest: &str) -> Result<bool, ExecutionError> {
        let Some(volume) = self.inspect_cache_volume(digest).await? else {
            self.compiler_cache.ready.lock().await.remove(digest);
            return Ok(false);
        };
        let generation = validate_cache_volume(&volume, digest)?;
        Ok(self
            .compiler_cache
            .ready
            .lock()
            .await
            .get(digest)
            .is_some_and(|ready_generation| ready_generation == &generation))
    }

    async fn inspect_cache_volume(&self, digest: &str) -> Result<Option<Volume>, ExecutionError> {
        match self
            .docker
            .inspect_volume(&compiler_cache_volume_name(digest))
            .await
        {
            Ok(volume) => Ok(Some(volume)),
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => Ok(None),
            Err(error) => Err(ExecutionError::Infrastructure(
                anyhow::Error::new(error).context("inspect compiler cache volume"),
            )),
        }
    }

    async fn probe_cached_compiler(
        &self,
        digest: &str,
        admission_permit: Option<Arc<OwnedSemaphorePermit>>,
    ) -> Result<bool, ExecutionError> {
        let container_name = format!("sc-verifier-compiler-cache-probe-{}", Uuid::new_v4());
        let family = ContainerFamily::CacheProbe;
        let created = {
            let _timer =
                metrics::start_operation(metrics::DOCKER, metrics::CREATE, family.as_str());
            self.docker
                .create_container(
                    Some(
                        CreateContainerOptionsBuilder::default()
                            .name(&container_name)
                            .platform(&self.settings.platform)
                            .build(),
                    ),
                    self.cache_helper_config(digest, false)?,
                )
                .await
                .context("create compiler cache probe")
                .map_err(ExecutionError::Infrastructure)?
        };
        let mut cleanup = ContainerCleanup::new(
            self.docker.clone(),
            created.id.clone(),
            family,
            admission_permit,
        );
        let result = self
            .run_created_container(&created.id, Bytes::new(), family)
            .await;
        let cleanup_result = cleanup
            .cleanup()
            .await
            .context("remove compiler cache probe");
        let output = preserve_primary_result(result, cleanup_result, &created.id, family)?;
        Ok(output.exit_code == 0)
    }

    async fn try_seed_compiler(
        &self,
        digest: &str,
        source: PathBuf,
        admission_permit: Option<Arc<OwnedSemaphorePermit>>,
    ) -> Result<SeedAttempt, ExecutionError> {
        let container_name = format!("{COMPILER_CACHE_INIT_PREFIX}{digest}");
        let family = ContainerFamily::CacheInitializer;
        let create_result = {
            let _timer =
                metrics::start_operation(metrics::DOCKER, metrics::CREATE, family.as_str());
            self.docker
                .create_container(
                    Some(
                        CreateContainerOptionsBuilder::default()
                            .name(&container_name)
                            .platform(&self.settings.platform)
                            .build(),
                    ),
                    self.cache_helper_config(digest, true)?,
                )
                .await
        };
        let created = match create_result {
            Ok(created) => created,
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 409, ..
            }) => return Ok(SeedAttempt::Busy),
            Err(error) => {
                return Err(ExecutionError::Infrastructure(
                    anyhow::Error::new(error).context("create compiler cache initializer"),
                ));
            }
        };
        let mut cleanup = ContainerCleanup::new(
            self.docker.clone(),
            created.id.clone(),
            family,
            admission_permit,
        );
        let max_upload_bytes = self.settings.max_upload_bytes;
        let expected_digest = digest.to_string();
        let archive = tokio::task::spawn_blocking(move || {
            let bytes = read_compiler_for_seed(&source, &expected_digest, max_upload_bytes)?;
            build_compiler_seed_archive(bytes, max_upload_bytes)
        })
        .await
        .context("join compiler cache archive task")
        .map_err(ExecutionError::Infrastructure)??;
        let archive_size = archive.len();
        let result = async {
            let _timer =
                metrics::start_operation(metrics::DOCKER, metrics::UPLOAD, family.as_str());
            timeout(
                Duration::from_secs(self.settings.execution_timeout_seconds),
                self.docker.upload_to_container(
                    &created.id,
                    Some(
                        UploadToContainerOptionsBuilder::default()
                            .path(COMPILER_CACHE_MOUNT)
                            .no_overwrite_dir_non_dir("true")
                            .build(),
                    ),
                    bollard::body_try_stream(archive.into_stream()),
                ),
            )
            .await
            .map_err(|_| ExecutionError::Timeout {
                seconds: self.settings.execution_timeout_seconds,
            })?
            .context("upload compiler into cache initializer")
            .map_err(ExecutionError::Infrastructure)?;
            metrics::add_transfer_bytes(
                metrics::DOCKER,
                metrics::INPUT,
                family.as_str(),
                archive_size,
            );
            drop(_timer);
            let output = self
                .run_created_container(&created.id, Bytes::new(), family)
                .await?;
            if output.exit_code != 0 {
                return Err(ExecutionError::Infrastructure(anyhow::anyhow!(
                    "compiler cache initializer rejected sha256:{digest}: {}",
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            Ok(())
        }
        .await;
        let cleanup_result = cleanup
            .cleanup()
            .await
            .context("remove compiler cache initializer");
        preserve_primary_result(result, cleanup_result, &created.id, family)?;
        Ok(SeedAttempt::Seeded)
    }

    async fn wait_for_cache_initializer(
        &self,
        digest: &str,
        deadline: Instant,
        cache_timeout_seconds: u64,
    ) -> Result<(), ExecutionError> {
        let name = format!("{COMPILER_CACHE_INIT_PREFIX}{digest}");
        loop {
            if Instant::now() >= deadline {
                return Err(ExecutionError::Timeout {
                    seconds: cache_timeout_seconds,
                });
            }
            match self.docker.inspect_container(&name, None).await {
                Ok(container) => {
                    if let Some(container_id) = abandoned_initializer_id(
                        &container,
                        unix_timestamp()?,
                        self.settings.request_timeout_seconds(),
                    ) {
                        force_remove_container(&self.docker, &container_id)
                            .await
                            .context("remove abandoned compiler cache initializer")
                            .map_err(ExecutionError::Infrastructure)?;
                        tracing::warn!(
                            container_id,
                            "removed abandoned compiler cache initializer"
                        );
                        return Ok(());
                    }
                }
                Err(bollard::errors::Error::DockerResponseServerError {
                    status_code: 404, ..
                }) => return Ok(()),
                Err(error) => {
                    return Err(ExecutionError::Infrastructure(
                        anyhow::Error::new(error).context("inspect compiler cache initializer"),
                    ));
                }
            }
            sleep(Duration::from_millis(250)).await;
        }
    }

    async fn run_created_container(
        &self,
        container_id: &str,
        stdin: Bytes,
        family: ContainerFamily,
    ) -> Result<ExecutionOutput, ExecutionError> {
        let _timer = metrics::start_operation(metrics::DOCKER, metrics::EXECUTE, family.as_str());
        let attached = self
            .docker
            .attach_container(
                container_id,
                Some(
                    AttachContainerOptionsBuilder::default()
                        .stdin(true)
                        .stdout(true)
                        .stderr(true)
                        .stream(true)
                        .logs(false)
                        .build(),
                ),
            )
            .await
            .context("attach to compiler container")
            .map_err(ExecutionError::Infrastructure)?;

        self.docker
            .start_container(container_id, None)
            .await
            .context("start compiler container")
            .map_err(ExecutionError::Infrastructure)?;

        let mut input = attached.input;
        let mut output = attached.output;
        let docker = self.docker.clone();
        let container_id = container_id.to_string();
        let wait_container_id = container_id.clone();
        let max_output_bytes = self.settings.max_output_bytes;
        let stdin_size = stdin.len();

        let run = async move {
            let write_stdin = async move {
                let fully_written = deliver_stdin(
                    &mut input,
                    &stdin,
                    "write compiler stdin",
                    "close compiler stdin",
                )
                .await?;
                if fully_written {
                    metrics::add_transfer_bytes(
                        metrics::DOCKER,
                        metrics::INPUT,
                        family.as_str(),
                        stdin_size,
                    );
                }
                Ok::<(), ExecutionError>(())
            };

            let collect_output = async move {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                while let Some(message) = output.next().await {
                    let message = message
                        .context("read compiler output")
                        .map_err(ExecutionError::Infrastructure)?;
                    let (is_stdout, bytes) = match message {
                        LogOutput::StdOut { message } => (true, message),
                        LogOutput::StdErr { message } => (false, message),
                        LogOutput::Console { message } => (false, message),
                        LogOutput::StdIn { .. } => continue,
                    };
                    metrics::add_transfer_bytes(
                        metrics::DOCKER,
                        metrics::OUTPUT,
                        family.as_str(),
                        bytes.len(),
                    );
                    if stdout.len() + stderr.len() + bytes.len() > max_output_bytes {
                        return Err(ExecutionError::OutputLimitExceeded {
                            limit: max_output_bytes,
                        });
                    }
                    if is_stdout {
                        stdout.extend_from_slice(&bytes);
                    } else {
                        stderr.extend_from_slice(&bytes);
                    }
                }
                Ok((Bytes::from(stdout), Bytes::from(stderr)))
            };

            let wait = async move {
                let mut stream = docker.wait_container(
                    &wait_container_id,
                    Some(WaitContainerOptionsBuilder::default().build()),
                );
                let response = stream
                    .next()
                    .await
                    .context("Docker wait stream ended without a response")
                    .map_err(ExecutionError::Infrastructure)?;
                match response {
                    Ok(response) => Ok(response.status_code),
                    Err(bollard::errors::Error::DockerContainerWaitError { code, .. }) => Ok(code),
                    Err(error) => Err(ExecutionError::Infrastructure(
                        anyhow::Error::new(error).context("wait for compiler container"),
                    )),
                }
            };

            finish_container_tasks(write_stdin, collect_output, wait).await
        };

        let (stdout, stderr, exit_code) = timeout(
            Duration::from_secs(self.settings.execution_timeout_seconds),
            run,
        )
        .await
        .map_err(|_| ExecutionError::Timeout {
            seconds: self.settings.execution_timeout_seconds,
        })??;

        let inspected = self
            .docker
            .inspect_container(&container_id, None)
            .await
            .context("inspect completed compiler container")
            .map_err(ExecutionError::Infrastructure)?;
        let oom_killed = inspected
            .state
            .and_then(|state| state.oom_killed)
            .unwrap_or(false);

        Ok(ExecutionOutput {
            stdout,
            stderr,
            exit_code,
            oom_killed,
        })
    }
}

async fn finish_container_tasks<Stdin, Output, Wait>(
    write_stdin: Stdin,
    collect_output: Output,
    wait: Wait,
) -> Result<(Bytes, Bytes, i64), ExecutionError>
where
    Stdin: std::future::Future<Output = Result<(), ExecutionError>>,
    Output: std::future::Future<Output = Result<(Bytes, Bytes), ExecutionError>>,
    Wait: std::future::Future<Output = Result<i64, ExecutionError>>,
{
    let (_, (stdout, stderr), exit_code) = tokio::try_join!(write_stdin, collect_output, wait)?;
    Ok((stdout, stderr, exit_code))
}

fn preserve_primary_result<T>(
    primary_result: Result<T, ExecutionError>,
    cleanup_result: anyhow::Result<()>,
    container_id: &str,
    family: ContainerFamily,
) -> Result<T, ExecutionError> {
    if let Err(error) = cleanup_result {
        tracing::warn!(
            container_id,
            family = family.as_str(),
            error = ?error,
            "eager compiler container cleanup failed; background retry remains armed"
        );
    }
    primary_result
}

#[async_trait]
impl CompilerExecutor for DockerCompilerExecutor {
    async fn execute(
        &self,
        invocation: CompilerInvocation,
    ) -> Result<ExecutionOutput, ExecutionError> {
        let observation = metrics::JobObservation::new(metrics::DOCKER, "job");
        let result = async {
            invocation.validate()?;
            let admission_permit = invocation.admission_permit();
            let max_upload_bytes = self.settings.max_upload_bytes;
            let prepare_invocation = invocation.clone();
            let compiler_cache = self.compiler_cache.clone();
            let prepare_timer = metrics::start_operation(metrics::DOCKER, metrics::PREPARE, "job");
            let (compilers, archive) = tokio::task::spawn_blocking(move || {
                let compilers = prepare_compilers(
                    &prepare_invocation,
                    max_upload_bytes,
                    &compiler_cache.local_digests,
                )?;
                let archive = build_archive(
                    &prepare_invocation,
                    max_upload_bytes,
                    compilers.logical_bytes,
                )?;
                Ok::<_, ExecutionError>((compilers, archive))
            })
            .await
            .context("join compiler archive task")
            .map_err(ExecutionError::Infrastructure)??;
            drop(prepare_timer);

            self.ensure_initialized().await?;

            let family = ContainerFamily::Job;
            let container_name = format!("sc-verifier-compiler-{}", Uuid::new_v4());
            let create_options = CreateContainerOptionsBuilder::default()
                .name(&container_name)
                .platform(&self.settings.platform)
                .build();
            let created = {
                let _timer =
                    metrics::start_operation(metrics::DOCKER, metrics::CREATE, family.as_str());
                self.docker
                    .create_container(
                        Some(create_options),
                        self.container_config(&invocation, &compilers)?,
                    )
                    .await
                    .context("create compiler container")
                    .map_err(ExecutionError::Infrastructure)?
            };
            let mut cleanup = ContainerCleanup::new(
                self.docker.clone(),
                created.id.clone(),
                family,
                admission_permit.clone(),
            );

            let archive_size = archive.len();
            let execution_result = async {
                // Creating the job first pins every named cache volume for the duration of cache
                // validation and execution. A missing volume is atomically recreated with verifier
                // ownership labels from the mount configuration, then seeded below before start.
                // One deadline covers the whole phase, including digest-lock waits, so the job
                // container cannot outlive the expiry label the janitor enforces.
                let cache_timeout_seconds = self
                    .settings
                    .cache_phase_timeout_seconds(compilers.by_digest.len());
                timeout(Duration::from_secs(cache_timeout_seconds), async {
                    for (digest, source) in &compilers.by_digest {
                        self.ensure_compiler_cached(digest, source, admission_permit.clone())
                            .await?;
                    }
                    Ok::<(), ExecutionError>(())
                })
                .await
                .map_err(|_| ExecutionError::Timeout {
                    seconds: cache_timeout_seconds,
                })??;

                // Executables are mounted from cache volumes, so a job without other files would
                // only upload an empty tar.
                if invocation.files().iter().any(|file| !file.is_executable()) {
                    let upload_timer =
                        metrics::start_operation(metrics::DOCKER, metrics::UPLOAD, family.as_str());
                    self.docker
                        .upload_to_container(
                            &created.id,
                            Some(
                                UploadToContainerOptionsBuilder::default()
                                    .path(JOB_ROOT)
                                    .no_overwrite_dir_non_dir("true")
                                    .build(),
                            ),
                            bollard::body_try_stream(archive.into_stream()),
                        )
                        .await
                        .context("upload compiler job files")
                        .map_err(ExecutionError::Infrastructure)?;
                    metrics::add_transfer_bytes(
                        metrics::DOCKER,
                        metrics::INPUT,
                        family.as_str(),
                        archive_size,
                    );
                    drop(upload_timer);
                }

                self.run_created_container(&created.id, invocation.stdin().clone(), family)
                    .await
            }
            .await;

            let cleanup_result = cleanup.cleanup().await.context("remove compiler container");

            preserve_primary_result(execution_result, cleanup_result, &created.id, family)
        }
        .await;
        observation.finish(&result);
        result
    }

    async fn health_check(&self) -> Result<(), ExecutionError> {
        if self.initialization.get().is_none() {
            self.ensure_initialized().await
        } else {
            self.check_remote_health().await
        }
    }
}

struct ContainerCleanup {
    docker: Docker,
    container_id: Option<String>,
    family: ContainerFamily,
    active: Option<metrics::GaugeGuard>,
    admission_permit: Option<Arc<OwnedSemaphorePermit>>,
}

struct ContainerJanitor {
    handle: tokio::task::JoinHandle<()>,
}

impl ContainerJanitor {
    fn spawn(docker: Docker) -> Self {
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(JANITOR_INTERVAL_SECONDS));
            // Remote initialization already performed the initial sweep.
            interval.tick().await;
            loop {
                interval.tick().await;
                if let Err(error) = reap_expired_containers(&docker, "periodic").await {
                    tracing::error!(error = ?error, "failed to reap expired compiler containers");
                }
            }
        });
        Self { handle }
    }
}

impl Drop for ContainerJanitor {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl ContainerCleanup {
    fn new(
        docker: Docker,
        container_id: String,
        family: ContainerFamily,
        admission_permit: Option<Arc<OwnedSemaphorePermit>>,
    ) -> Self {
        Self {
            docker,
            container_id: Some(container_id),
            family,
            active: Some(metrics::observe_container(family)),
            admission_permit,
        }
    }

    async fn cleanup(&mut self) -> anyhow::Result<()> {
        let Some(container_id) = self.container_id.as_deref() else {
            return Ok(());
        };
        let _timer =
            metrics::start_operation(metrics::DOCKER, metrics::CLEANUP, self.family.as_str());
        let result = force_remove_container(&self.docker, container_id).await;
        if result.is_ok() {
            self.container_id = None;
            drop(self.active.take());
            drop(self.admission_permit.take());
        }
        result
    }
}

impl Drop for ContainerCleanup {
    fn drop(&mut self) {
        let Some(container_id) = self.container_id.take() else {
            return;
        };
        let docker = self.docker.clone();
        let family = self.family;
        let active = self.active.take();
        let admission_permit = self.admission_permit.take();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _timer =
                    metrics::start_operation(metrics::DOCKER, metrics::CLEANUP, family.as_str());
                if let Err(error) = force_remove_container(&docker, &container_id).await {
                    tracing::error!(
                        container_id,
                        error = ?error,
                        "failed to clean up compiler container"
                    );
                }
                drop(active);
                drop(admission_permit);
            });
        } else {
            tracing::error!(
                container_id,
                "could not schedule compiler container cleanup without a Tokio runtime"
            );
        }
    }
}

async fn reap_expired_containers(docker: &Docker, trigger: &'static str) -> anyhow::Result<()> {
    let result = reap_expired_containers_inner(docker).await;
    metrics::count_orphan_sweep(trigger, result.is_ok());
    result
}

async fn reap_expired_containers_inner(docker: &Docker) -> anyhow::Result<()> {
    let filters = HashMap::from([("label".to_string(), vec![JOB_LABEL.to_string()])]);
    let options = ListContainersOptionsBuilder::default()
        .all(true)
        .filters(&filters)
        .build();
    let now = unix_timestamp()?;
    let mut first_error = None;
    for container in docker.list_containers(Some(options)).await? {
        let family = observed_container_family(container.labels.as_ref());
        let expiry = container
            .labels
            .as_ref()
            .and_then(|labels| labels.get(JOB_EXPIRY_LABEL))
            .and_then(|expiry| expiry.parse::<u64>().ok());
        let Some(expiry) = expiry else {
            metrics::count_orphan_container(family, "invalid_expiry");
            continue;
        };
        if expiry > now {
            continue;
        }
        let Some(container_id) = container.id else {
            metrics::count_orphan_container(family, "missing_id");
            continue;
        };
        let _timer = metrics::start_operation(metrics::DOCKER, metrics::CLEANUP, family);
        match force_remove_container(docker, &container_id).await {
            Ok(()) => {
                metrics::count_orphan_container(family, "removed_or_missing");
                tracing::warn!(container_id, "removed expired compiler container");
            }
            Err(error) => {
                metrics::count_orphan_container(family, "remove_error");
                let error =
                    error.context(format!("remove expired compiler container {container_id}"));
                tracing::error!(container_id, error = ?error, "failed to remove expired compiler container");
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

fn observed_container_family(labels: Option<&HashMap<String, String>>) -> &'static str {
    match labels.and_then(|labels| labels.get(CONTAINER_FAMILY_LABEL).map(String::as_str)) {
        Some("job") => ContainerFamily::Job.as_str(),
        Some("cache_probe") => ContainerFamily::CacheProbe.as_str(),
        Some("cache_initializer") => ContainerFamily::CacheInitializer.as_str(),
        _ => "unknown",
    }
}

/// Returns the id of an initializer whose owner stopped it but never removed it. The grace period
/// leaves a live owner time to inspect the exit state before its own cleanup removes the container.
fn abandoned_initializer_id(
    container: &ContainerInspectResponse,
    now: u64,
    grace_seconds: u64,
) -> Option<String> {
    let state = container.state.as_ref()?;
    if !matches!(
        state.status,
        Some(ContainerStateStatusEnum::EXITED | ContainerStateStatusEnum::DEAD)
    ) {
        return None;
    }
    let finished_at = chrono::DateTime::parse_from_rfc3339(state.finished_at.as_deref()?).ok()?;
    let finished_at = u64::try_from(finished_at.timestamp()).ok()?;
    if now.saturating_sub(finished_at) < grace_seconds {
        return None;
    }
    container.id.clone()
}

async fn force_remove_container(docker: &Docker, container_id: &str) -> anyhow::Result<()> {
    let options = RemoveContainerOptionsBuilder::default()
        .force(true)
        .v(true)
        .build();
    match docker.remove_container(container_id, Some(options)).await {
        Ok(())
        | Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn unix_timestamp() -> Result<u64, ExecutionError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")
        .map(|duration| duration.as_secs())
        .map_err(ExecutionError::Infrastructure)
}

fn validate_settings(settings: &DockerCompilerExecutorSettings) -> Result<(), ExecutionError> {
    let addr = Url::parse(&settings.addr).map_err(|error| {
        ExecutionError::InvalidInvocation(format!("invalid Docker address: {error}"))
    })?;
    if addr.scheme() != "ssh" {
        return Err(ExecutionError::InvalidInvocation(
            "compiler Docker address must use ssh://".to_string(),
        ));
    }
    if addr.host_str().is_none()
        || addr.password().is_some()
        || !addr.path().is_empty()
        || addr.query().is_some()
        || addr.fragment().is_some()
    {
        return Err(ExecutionError::InvalidInvocation(
            "compiler Docker SSH address must contain only user, host, and optional port"
                .to_string(),
        ));
    }
    let digest_is_pinned =
        settings
            .runner_image
            .rsplit_once("@sha256:")
            .is_some_and(|(name, digest)| {
                !name.is_empty()
                    && digest.len() == 64
                    && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            });
    if !digest_is_pinned {
        return Err(ExecutionError::InvalidInvocation(
            "compiler runner image must be pinned by sha256 digest".to_string(),
        ));
    }
    if settings.platform.is_empty() {
        return Err(ExecutionError::InvalidInvocation(
            "compiler runner platform must not be empty".to_string(),
        ));
    }
    if !settings.platform.starts_with("linux/") {
        return Err(ExecutionError::InvalidInvocation(
            "compiler runner platform must be a Linux platform".to_string(),
        ));
    }
    if settings.connect_timeout_seconds == 0
        || settings.api_timeout_seconds == 0
        || settings.execution_timeout_seconds == 0
        || settings.memory_limit_bytes <= 0
        || settings.nano_cpus <= 0
        || settings.pids_limit <= 0
        || settings.max_upload_bytes == 0
        || settings.max_output_bytes == 0
    {
        return Err(ExecutionError::InvalidInvocation(
            "compiler Docker limits and timeouts must be positive".to_string(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SeedAttempt {
    Seeded,
    Busy,
}

fn compiler_cache_volume_name(digest: &str) -> String {
    format!("{COMPILER_CACHE_VOLUME_PREFIX}{digest}")
}

fn compiler_cache_mount_path(digest: &str) -> PathBuf {
    Path::new(COMPILER_CACHE_ROOT)
        .join(digest)
        .join(COMPILER_CACHE_FILE)
}

fn compiler_cache_labels(digest: &str) -> HashMap<String, String> {
    HashMap::from([
        (COMPILER_CACHE_LABEL.to_string(), "true".to_string()),
        (COMPILER_CACHE_SCHEMA_LABEL.to_string(), "1".to_string()),
        (
            COMPILER_CACHE_DIGEST_LABEL.to_string(),
            format!("sha256:{digest}"),
        ),
    ])
}

fn new_compiler_cache_labels(digest: &str) -> HashMap<String, String> {
    let mut labels = compiler_cache_labels(digest);
    labels.insert(
        COMPILER_CACHE_GENERATION_LABEL.to_string(),
        Uuid::new_v4().to_string(),
    );
    labels
}

fn compiler_cache_mount_options(digest: &str) -> MountVolumeOptions {
    MountVolumeOptions {
        no_copy: Some(true),
        labels: Some(new_compiler_cache_labels(digest)),
        driver_config: Some(MountVolumeOptionsDriverConfig {
            name: Some("local".to_string()),
            options: None,
        }),
        ..Default::default()
    }
}

/// Returns the volume's generation, which identifies one incarnation of the cache volume.
fn validate_cache_volume(volume: &Volume, digest: &str) -> Result<String, ExecutionError> {
    let expected_name = compiler_cache_volume_name(digest);
    let mut expected_labels = compiler_cache_labels(digest);
    let generation = volume
        .labels
        .get(COMPILER_CACHE_GENERATION_LABEL)
        .filter(|generation| Uuid::parse_str(generation).is_ok())
        .cloned();
    if let Some(generation) = generation.as_ref() {
        expected_labels.insert(
            COMPILER_CACHE_GENERATION_LABEL.to_string(),
            generation.clone(),
        );
    }
    match generation {
        Some(generation)
            if volume.name == expected_name
                && volume.driver == "local"
                && volume.options.is_empty()
                && volume.labels == expected_labels =>
        {
            Ok(generation)
        }
        _ => Err(ExecutionError::Infrastructure(anyhow::anyhow!(
            "compiler cache volume {expected_name} has unexpected ownership metadata; expected the local driver, no driver options, and exact verifier cache labels including a generation. Remove it if an older verifier build created it"
        ))),
    }
}

fn prepare_compilers(
    invocation: &CompilerInvocation,
    max_upload_bytes: usize,
    local_digests: &LocalDigestCache,
) -> Result<PreparedCompilers, ExecutionError> {
    let mut logical_bytes = invocation.stdin().len();
    if logical_bytes > max_upload_bytes {
        return Err(ExecutionError::UploadLimitExceeded {
            limit: max_upload_bytes as u64,
        });
    }
    let mut by_digest = BTreeMap::new();
    let mut path_overrides = HashMap::new();
    for file in invocation
        .files()
        .iter()
        .filter(|file| file.is_executable())
    {
        let JobFileContent::LocalPath(path) = file.content() else {
            return Err(ExecutionError::InvalidInvocation(format!(
                "Docker compiler executable must come from a local regular file: {}",
                file.path().display()
            )));
        };
        let metadata = std::fs::symlink_metadata(path)
            .with_context(|| format!("inspect compiler executable {}", path.display()))
            .map_err(ExecutionError::Infrastructure)?;
        if !metadata.file_type().is_file() {
            return Err(ExecutionError::InvalidInvocation(format!(
                "compiler executable source must be a regular file: {}",
                path.display()
            )));
        }
        let size = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
        if size > max_upload_bytes.saturating_sub(logical_bytes) {
            return Err(ExecutionError::UploadLimitExceeded {
                limit: max_upload_bytes as u64,
            });
        }
        logical_bytes = logical_bytes.saturating_add(size);
        let digest = local_digests.digest(path, &metadata)?;
        path_overrides.insert(file.id().to_string(), compiler_cache_mount_path(&digest));
        by_digest.entry(digest).or_insert_with(|| path.clone());
    }
    Ok(PreparedCompilers {
        by_digest,
        path_overrides,
        logical_bytes,
    })
}

#[derive(Debug)]
struct ArchiveLimitExceeded;

impl std::fmt::Display for ArchiveLimitExceeded {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("compiler upload archive exceeds its configured limit")
    }
}

impl std::error::Error for ArchiveLimitExceeded {}

struct BoundedArchiveWriter {
    file: std::fs::File,
    len: usize,
    limit: usize,
}

impl BoundedArchiveWriter {
    fn new(limit: usize) -> Result<Self, ExecutionError> {
        let file = tempfile::tempfile()
            .context("create compiler upload spool")
            .map_err(ExecutionError::Infrastructure)?;
        Ok(Self {
            file,
            len: 0,
            limit,
        })
    }

    fn into_archive(mut self) -> Result<SpooledArchive, ExecutionError> {
        self.file
            .flush()
            .context("flush compiler upload spool")
            .map_err(ExecutionError::Infrastructure)?;
        self.file
            .seek(SeekFrom::Start(0))
            .context("rewind compiler upload spool")
            .map_err(ExecutionError::Infrastructure)?;
        Ok(SpooledArchive {
            file: self.file,
            len: self.len,
        })
    }
}

impl Write for BoundedArchiveWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.len) {
            return Err(std::io::Error::other(ArchiveLimitExceeded));
        }
        let written = self.file.write(bytes)?;
        self.len = self.len.saturating_add(written);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

struct ExactSizeReader<R> {
    inner: R,
    remaining: u64,
}

impl<R> ExactSizeReader<R> {
    fn new(inner: R, size: u64) -> Self {
        Self {
            inner,
            remaining: size,
        }
    }
}

impl<R: Read> Read for ExactSizeReader<R> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 || output.is_empty() {
            return Ok(0);
        }
        let requested = output
            .len()
            .min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let read = self.inner.read(&mut output[..requested])?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "compiler job file changed while its archive was being built",
            ));
        }
        self.remaining -= read as u64;
        Ok(read)
    }
}

fn map_archive_error(
    error: std::io::Error,
    limit: usize,
    context: impl Into<String>,
) -> ExecutionError {
    if error
        .get_ref()
        .and_then(|source| source.downcast_ref::<ArchiveLimitExceeded>())
        .is_some()
    {
        ExecutionError::UploadLimitExceeded {
            limit: limit as u64,
        }
    } else {
        ExecutionError::Infrastructure(anyhow::Error::new(error).context(context.into()))
    }
}

/// Reads a compiler only on a cache miss and checks it still has the digest its volume is named by.
fn read_compiler_for_seed(
    source: &Path,
    expected_digest: &str,
    max_upload_bytes: usize,
) -> Result<Bytes, ExecutionError> {
    let read_limit = u64::try_from(max_upload_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::new();
    std::fs::File::open(source)
        .and_then(|file| file.take(read_limit).read_to_end(&mut bytes))
        .with_context(|| format!("read compiler executable {}", source.display()))
        .map_err(ExecutionError::Infrastructure)?;
    if bytes.len() > max_upload_bytes {
        return Err(ExecutionError::UploadLimitExceeded {
            limit: max_upload_bytes as u64,
        });
    }
    if hex::encode(Sha256::digest(&bytes)) != expected_digest {
        return Err(ExecutionError::Infrastructure(anyhow::anyhow!(
            "compiler executable {} changed after it was hashed",
            source.display()
        )));
    }
    Ok(Bytes::from(bytes))
}

fn build_compiler_seed_archive(
    bytes: Bytes,
    max_upload_bytes: usize,
) -> Result<SpooledArchive, ExecutionError> {
    let mut archive = BoundedArchiveWriter::new(max_upload_bytes)?;
    let mut builder = tar::Builder::new(&mut archive);
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o444);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_cksum();
    builder
        .append_data(
            &mut header,
            ".incoming",
            ExactSizeReader::new(Cursor::new(bytes.clone()), bytes.len() as u64),
        )
        .map_err(|error| {
            map_archive_error(
                error,
                max_upload_bytes,
                "append compiler to cache seed archive",
            )
        })?;
    builder.finish().map_err(|error| {
        map_archive_error(
            error,
            max_upload_bytes,
            "finish compiler cache seed archive",
        )
    })?;
    drop(builder);
    archive.into_archive()
}

fn build_archive(
    invocation: &CompilerInvocation,
    max_upload_bytes: usize,
    compiler_bytes: usize,
) -> Result<SpooledArchive, ExecutionError> {
    let mut uploaded_bytes = compiler_bytes;
    if uploaded_bytes > max_upload_bytes {
        return Err(ExecutionError::UploadLimitExceeded {
            limit: max_upload_bytes as u64,
        });
    }

    let archive_limit = max_upload_bytes.saturating_sub(invocation.stdin().len());
    let mut archive = BoundedArchiveWriter::new(archive_limit)?;
    let mut builder = tar::Builder::new(&mut archive);
    for file in invocation.files() {
        if file.is_executable() {
            continue;
        }
        let (size, content): (u64, Box<dyn std::io::Read>) = match file.content() {
            JobFileContent::Bytes(bytes) => {
                uploaded_bytes = uploaded_bytes.saturating_add(bytes.len());
                (
                    bytes.len() as u64,
                    Box::new(ExactSizeReader::new(
                        Cursor::new(bytes.clone()),
                        bytes.len() as u64,
                    )),
                )
            }
            JobFileContent::LocalPath(path) => {
                let metadata = std::fs::symlink_metadata(path)
                    .with_context(|| format!("inspect compiler job file {}", path.display()))
                    .map_err(ExecutionError::Infrastructure)?;
                if !metadata.file_type().is_file() {
                    return Err(ExecutionError::InvalidInvocation(format!(
                        "compiler job source must be a regular file: {}",
                        path.display()
                    )));
                }
                let size = usize::try_from(metadata.len()).map_err(|_| {
                    ExecutionError::UploadLimitExceeded {
                        limit: max_upload_bytes as u64,
                    }
                })?;
                uploaded_bytes = uploaded_bytes.saturating_add(size);
                let source = std::fs::File::open(path)
                    .with_context(|| format!("open compiler job file {}", path.display()))
                    .map_err(ExecutionError::Infrastructure)?;
                (
                    metadata.len(),
                    Box::new(ExactSizeReader::new(source, metadata.len())),
                )
            }
        };
        if uploaded_bytes > max_upload_bytes {
            return Err(ExecutionError::UploadLimitExceeded {
                limit: max_upload_bytes as u64,
            });
        }

        let mut header = tar::Header::new_gnu();
        header.set_size(size);
        header.set_mode(file.mode());
        // The compiler user may read/execute staged files, but cannot chmod or grow them.
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_cksum();
        builder
            .append_data(&mut header, file.path(), content)
            .map_err(|error| {
                map_archive_error(
                    error,
                    max_upload_bytes,
                    format!(
                        "append compiler job file {} to archive",
                        file.path().display()
                    ),
                )
            })?;
    }
    builder.finish().map_err(|error| {
        map_archive_error(error, max_upload_bytes, "finish compiler job archive")
    })?;
    drop(builder);
    archive.into_archive()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compiler::JobFile, CommandArgument, DetailedVersion, Fetcher, ListFetcher};
    use std::{
        future::Future,
        io::Read,
        marker::PhantomData,
        pin::Pin,
        str::FromStr,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        task::{Context as TaskContext, Poll},
    };

    const LINUX_AMD64_VYPER_COMPILER_LIST: &str =
        "https://raw.githubusercontent.com/blockscout/solc-bin/main/vyper.list.json";

    struct PendingUntilDropped<T> {
        dropped: Arc<AtomicBool>,
        output: PhantomData<T>,
    }

    impl<T> PendingUntilDropped<T> {
        fn new(dropped: Arc<AtomicBool>) -> Self {
            Self {
                dropped,
                output: PhantomData,
            }
        }
    }

    impl<T> Future for PendingUntilDropped<T> {
        type Output = T;

        fn poll(self: Pin<&mut Self>, _context: &mut TaskContext<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }

    impl<T> Drop for PendingUntilDropped<T> {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    fn settings() -> DockerCompilerExecutorSettings {
        DockerCompilerExecutorSettings {
            addr: "ssh://docker@example.test".to_string(),
            key_path: None,
            runner_image: format!("compiler-runner@sha256:{}", "0".repeat(64)),
            platform: "linux/amd64".to_string(),
            connect_timeout_seconds: 10,
            api_timeout_seconds: 10,
            execution_timeout_seconds: 30,
            memory_limit_bytes: 512 * 1024 * 1024,
            nano_cpus: 1_000_000_000,
            pids_limit: 64,
            max_upload_bytes: 64 * 1024 * 1024,
            max_output_bytes: 8 * 1024 * 1024,
            runtime: None,
        }
    }

    #[test]
    fn lifecycle_budget_uses_the_effective_request_timeout() {
        let mut settings = settings();
        assert_eq!(settings.request_timeout_seconds(), 30);
        assert_eq!(settings.container_lifecycle_timeout_seconds(), 90);
        assert_eq!(settings.job_container_lifecycle_timeout_seconds(1), 180);
        assert_eq!(settings.job_container_lifecycle_timeout_seconds(2), 270);

        settings.api_timeout_seconds = 45;
        assert_eq!(settings.request_timeout_seconds(), 45);
        assert_eq!(settings.container_lifecycle_timeout_seconds(), 120);
        assert_eq!(settings.job_container_lifecycle_timeout_seconds(2), 360);
        // The job container expiry covers the full cache phase plus one container run.
        assert_eq!(settings.cache_phase_timeout_seconds(2), 240);
        assert_eq!(
            settings.job_container_lifecycle_timeout_seconds(2),
            settings.cache_phase_timeout_seconds(2)
                + settings.container_lifecycle_timeout_seconds()
        );
    }

    #[tokio::test]
    async fn output_limit_cancels_pending_container_tasks() {
        let stdin_dropped = Arc::new(AtomicBool::new(false));
        let wait_dropped = Arc::new(AtomicBool::new(false));

        let error = timeout(
            Duration::from_millis(100),
            finish_container_tasks(
                PendingUntilDropped::<Result<(), ExecutionError>>::new(stdin_dropped.clone()),
                async {
                    Err::<(Bytes, Bytes), ExecutionError>(ExecutionError::OutputLimitExceeded {
                        limit: 16,
                    })
                },
                PendingUntilDropped::<Result<i64, ExecutionError>>::new(wait_dropped.clone()),
            ),
        )
        .await
        .expect("output-limit error must not wait for the other container tasks")
        .unwrap_err();

        assert!(matches!(
            error,
            ExecutionError::OutputLimitExceeded { limit: 16 }
        ));
        assert!(stdin_dropped.load(Ordering::SeqCst));
        assert!(wait_dropped.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn successful_container_tasks_preserve_all_outputs() {
        let output = finish_container_tasks(
            async { Ok(()) },
            async {
                Ok((
                    Bytes::from_static(b"compiler output"),
                    Bytes::from_static(b"compiler warning"),
                ))
            },
            async { Ok(7) },
        )
        .await
        .unwrap();

        assert_eq!(output.0, Bytes::from_static(b"compiler output"));
        assert_eq!(output.1, Bytes::from_static(b"compiler warning"));
        assert_eq!(output.2, 7);
    }

    #[test]
    fn cleanup_failure_does_not_discard_completed_compiler_output() {
        let expected = ExecutionOutput {
            stdout: Bytes::from_static(b"compiler output"),
            stderr: Bytes::from_static(b"compiler diagnostics"),
            exit_code: 7,
            oom_killed: false,
        };

        let output = preserve_primary_result(
            Ok(expected),
            Err(anyhow::anyhow!("transient Docker cleanup failure")),
            "container-id",
            ContainerFamily::Job,
        )
        .expect("cleanup must not replace a completed compiler result");

        assert_eq!(output.stdout, Bytes::from_static(b"compiler output"));
        assert_eq!(output.stderr, Bytes::from_static(b"compiler diagnostics"));
        assert_eq!(output.exit_code, 7);
        assert!(!output.oom_killed);

        let output = preserve_primary_result(Ok(42), Ok(()), "container-id", ContainerFamily::Job)
            .expect("successful cleanup must preserve the primary result");
        assert_eq!(output, 42);
    }

    #[test]
    fn primary_error_wins_when_cleanup_also_fails() {
        let error = preserve_primary_result::<()>(
            Err(ExecutionError::Timeout { seconds: 37 }),
            Err(anyhow::anyhow!("transient Docker cleanup failure")),
            "container-id",
            ContainerFamily::CacheInitializer,
        )
        .unwrap_err();

        assert!(matches!(error, ExecutionError::Timeout { seconds: 37 }));
    }

    #[tokio::test]
    async fn expired_cache_wait_is_classified_as_timeout() {
        let docker =
            Docker::connect_with_ssh("ssh://docker@example.test", 10, API_DEFAULT_VERSION, None)
                .unwrap();
        let executor = DockerCompilerExecutor {
            docker,
            settings: Arc::new(settings()),
            compiler_cache: Arc::new(CompilerCacheState::default()),
            initialization: Arc::new(OnceCell::new()),
        };

        let error = executor
            .wait_for_cache_initializer(&"0".repeat(64), Instant::now(), 90)
            .await
            .unwrap_err();
        assert!(matches!(error, ExecutionError::Timeout { seconds: 90 }));
    }

    #[test]
    fn rejects_invalid_connection_settings() {
        let mut invalid = settings();
        invalid.addr = "tcp://docker.example.test:2375".to_string();
        assert!(validate_settings(&invalid).is_err());
        assert!(DockerCompilerExecutor::new(invalid).is_err());

        let mut invalid = settings();
        invalid.runner_image = "compiler-runner:latest".to_string();
        assert!(validate_settings(&invalid).is_err());

        let mut invalid = settings();
        invalid.connect_timeout_seconds = 0;
        assert!(validate_settings(&invalid).is_err());

        let mut invalid = settings();
        invalid.api_timeout_seconds = 0;
        assert!(validate_settings(&invalid).is_err());
    }

    #[test]
    fn lazy_executor_construction_does_not_contact_remote_host() {
        let executor = DockerCompilerExecutor::new(settings())
            .expect("a valid but unreachable host must not fail construction");

        assert!(executor.initialization.get().is_none());
    }

    #[test]
    fn generated_container_is_locked_down() {
        let docker =
            Docker::connect_with_ssh("ssh://docker@example.test", 10, API_DEFAULT_VERSION, None)
                .unwrap();
        let executor = DockerCompilerExecutor {
            docker,
            settings: Arc::new(settings()),
            compiler_cache: Arc::new(CompilerCacheState::default()),
            initialization: Arc::new(OnceCell::new()),
        };
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        std::fs::write(&compiler, b"compiler binary").unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::new(),
        );
        let compilers =
            prepare_compilers(&invocation, 1024 * 1024, &LocalDigestCache::default()).unwrap();

        let config = executor.container_config(&invocation, &compilers).unwrap();
        assert_eq!(
            serde_json::to_value(&config).unwrap()["Entrypoint"],
            serde_json::json!([""])
        );
        assert_eq!(config.entrypoint, Some(vec![String::new()]));
        let digest = compilers.by_digest.keys().next().unwrap();
        assert_eq!(
            config.cmd,
            Some(vec![compiler_cache_mount_path(digest)
                .to_string_lossy()
                .into_owned()])
        );
        assert_eq!(config.user.as_deref(), Some(JOB_USER));
        assert_eq!(config.network_disabled, Some(true));
        assert_eq!(
            config.env.as_deref(),
            Some(
                [
                    format!("HOME={HOME_DIR}"),
                    format!("TMPDIR={COMPILER_TMP_DIR}"),
                ]
                .as_slice()
            )
        );
        let expiry = config
            .labels
            .as_ref()
            .and_then(|labels| labels.get(JOB_EXPIRY_LABEL))
            .and_then(|expiry| expiry.parse::<u64>().ok())
            .unwrap();
        assert!(expiry > unix_timestamp().unwrap());
        assert_eq!(
            config
                .labels
                .as_ref()
                .and_then(|labels| labels.get(CONTAINER_FAMILY_LABEL))
                .map(String::as_str),
            Some("job")
        );
        let host = config.host_config.unwrap();
        assert_eq!(host.network_mode.as_deref(), Some("none"));
        assert_eq!(host.privileged, Some(false));
        assert_eq!(host.readonly_rootfs, Some(true));
        assert_eq!(host.cap_drop, Some(vec!["ALL".to_string()]));
        assert_eq!(
            host.log_config.and_then(|config| config.typ),
            Some("none".to_string())
        );
        assert!(host.binds.is_none());
        let tmpfs = host.tmpfs.as_ref().unwrap();
        assert_eq!(
            tmpfs.get(HOME_DIR).map(String::as_str),
            Some("rw,noexec,nosuid,nodev,size=67108864")
        );
        assert_eq!(
            tmpfs.get(COMPILER_TMP_DIR).map(String::as_str),
            Some("rw,exec,nosuid,nodev,size=268435456,uid=65532,gid=65532,mode=0700")
        );
        let mounts = host.mounts.unwrap();
        assert_eq!(mounts.len(), 2);
        assert_eq!(mounts[0].typ, Some(MountType::VOLUME));
        assert!(mounts[0].source.is_none());
        assert_eq!(
            mounts[0]
                .volume_options
                .as_ref()
                .and_then(|options| options.no_copy),
            Some(true)
        );
        assert_eq!(mounts[1].source, Some(compiler_cache_volume_name(digest)));
        assert_eq!(mounts[1].read_only, Some(true));
        let cache_labels = mounts[1]
            .volume_options
            .as_ref()
            .and_then(|options| options.labels.as_ref())
            .unwrap();
        assert_eq!(
            cache_labels.get(COMPILER_CACHE_LABEL).map(String::as_str),
            Some("true")
        );
        assert_eq!(
            cache_labels
                .get(COMPILER_CACHE_DIGEST_LABEL)
                .map(String::as_str),
            Some(format!("sha256:{digest}").as_str())
        );
        assert!(cache_labels
            .get(COMPILER_CACHE_GENERATION_LABEL)
            .is_some_and(|generation| !generation.is_empty()));
        let cache_driver = mounts[1]
            .volume_options
            .as_ref()
            .and_then(|options| options.driver_config.as_ref())
            .unwrap();
        assert_eq!(cache_driver.name.as_deref(), Some("local"));
        assert!(cache_driver.options.is_none());
        assert_eq!(
            mounts[1].target,
            Some(
                compiler_cache_mount_path(digest)
                    .parent()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            )
        );
        assert!(host.devices.is_none());
    }

    #[test]
    fn cache_helpers_have_bounded_family_labels() {
        let docker =
            Docker::connect_with_ssh("ssh://docker@example.test", 10, API_DEFAULT_VERSION, None)
                .unwrap();
        let executor = DockerCompilerExecutor {
            docker,
            settings: Arc::new(settings()),
            compiler_cache: Arc::new(CompilerCacheState::default()),
            initialization: Arc::new(OnceCell::new()),
        };

        for (initialize, expected) in [(false, "cache_probe"), (true, "cache_initializer")] {
            let config = executor
                .cache_helper_config(&"0".repeat(64), initialize)
                .unwrap();
            assert_eq!(
                serde_json::to_value(&config).unwrap()["Entrypoint"],
                serde_json::json!([""])
            );
            assert_eq!(config.entrypoint, Some(vec![String::new()]));
            assert_eq!(
                config
                    .cmd
                    .as_ref()
                    .and_then(|command| command.first())
                    .map(String::as_str),
                Some("/bin/sh")
            );
            assert_eq!(
                config
                    .labels
                    .as_ref()
                    .and_then(|labels| labels.get(CONTAINER_FAMILY_LABEL))
                    .map(String::as_str),
                Some(expected)
            );
            let volume_options = config
                .host_config
                .as_ref()
                .and_then(|host| host.mounts.as_ref())
                .and_then(|mounts| mounts.first())
                .and_then(|mount| mount.volume_options.as_ref())
                .unwrap();
            assert_eq!(volume_options.no_copy, Some(true));
            assert!(volume_options
                .labels
                .as_ref()
                .and_then(|labels| labels.get(COMPILER_CACHE_GENERATION_LABEL))
                .is_some_and(|generation| Uuid::parse_str(generation).is_ok()));
            assert_eq!(
                volume_options
                    .driver_config
                    .as_ref()
                    .and_then(|driver| driver.name.as_deref()),
                Some("local")
            );
        }
    }

    #[test]
    fn only_stopped_initializers_past_grace_are_abandoned() {
        let finished_at = "2026-01-01T00:00:00.123456789Z";
        let finished = chrono::DateTime::parse_from_rfc3339(finished_at)
            .unwrap()
            .timestamp() as u64;
        let container = |status| ContainerInspectResponse {
            id: Some("initializer-id".to_string()),
            state: Some(bollard::models::ContainerState {
                status: Some(status),
                finished_at: Some(finished_at.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        for status in [
            ContainerStateStatusEnum::EXITED,
            ContainerStateStatusEnum::DEAD,
        ] {
            assert_eq!(
                abandoned_initializer_id(&container(status), finished + 30, 30).as_deref(),
                Some("initializer-id")
            );
            assert_eq!(
                abandoned_initializer_id(&container(status), finished + 29, 30),
                None
            );
        }
        for status in [
            ContainerStateStatusEnum::CREATED,
            ContainerStateStatusEnum::RUNNING,
        ] {
            assert_eq!(
                abandoned_initializer_id(&container(status), finished + 3600, 30),
                None
            );
        }
    }

    #[test]
    fn orphan_family_labels_fail_to_unknown() {
        for (label, expected) in [
            (Some("job"), "job"),
            (Some("cache_probe"), "cache_probe"),
            (Some("cache_initializer"), "cache_initializer"),
            (Some("arbitrary"), "unknown"),
            (None, "unknown"),
        ] {
            let labels = label.map(|label| {
                HashMap::from([(CONTAINER_FAMILY_LABEL.to_string(), label.to_string())])
            });
            assert_eq!(observed_container_family(labels.as_ref()), expected);
        }
    }

    #[test]
    fn archive_contains_only_normalized_regular_files() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        let source = source_dir.path().join("input.sol");
        std::fs::write(&compiler, b"binary").unwrap();
        std::fs::write(&source, b"contract C {}").unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::new(),
        )
        .with_file(JobFile::regular_file("input", "sources/input.sol", source).unwrap());

        let compilers =
            prepare_compilers(&invocation, 1024 * 1024, &LocalDigestCache::default()).unwrap();
        let archive = build_archive(&invocation, 1024 * 1024, compilers.logical_bytes).unwrap();
        let mut archive = tar::Archive::new(archive.file);
        let entries = archive.entries().unwrap();
        let mut names = Vec::new();
        for entry in entries {
            let mut entry = entry.unwrap();
            assert!(entry.header().entry_type().is_file());
            assert_eq!(entry.header().uid().unwrap(), 0);
            assert_eq!(entry.header().gid().unwrap(), 0);
            names.push(entry.path().unwrap().to_string_lossy().into_owned());
            let mut ignored = Vec::new();
            entry.read_to_end(&mut ignored).unwrap();
        }
        assert_eq!(names, vec!["sources/input.sol"]);
    }

    #[test]
    fn executable_cache_is_content_addressed_and_deduplicated() {
        let source_dir = tempfile::tempdir().unwrap();
        let zksolc = source_dir.path().join("zksolc");
        let solc = source_dir.path().join("solc");
        std::fs::write(&zksolc, b"abc").unwrap();
        std::fs::write(&solc, b"abc").unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("zksolc", "bin/zksolc", zksolc).unwrap(),
            vec![CommandArgument::prefixed_file("--solc=", "solc")],
            Bytes::new(),
        )
        .with_file(JobFile::executable("solc", "bin/solc", solc).unwrap());

        let compilers = prepare_compilers(&invocation, 1024, &LocalDigestCache::default()).unwrap();
        let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(compilers.by_digest.len(), 1);
        assert_eq!(
            std::fs::read(compilers.by_digest.get(digest).unwrap()).unwrap(),
            b"abc"
        );
        let cached_path = compiler_cache_mount_path(digest);
        assert_eq!(compilers.path_overrides.get("zksolc"), Some(&cached_path));
        assert_eq!(compilers.path_overrides.get("solc"), Some(&cached_path));
        assert_eq!(
            invocation
                .program_path_with_overrides(Path::new(JOB_ROOT), &compilers.path_overrides)
                .unwrap(),
            cached_path
        );
        assert_eq!(
            invocation
                .resolved_args_with_overrides(Path::new(JOB_ROOT), &compilers.path_overrides)
                .unwrap(),
            vec![format!("--solc={}", cached_path.display())]
        );
        assert_eq!(compilers.logical_bytes, 6);
    }

    #[test]
    fn unchanged_compiler_reuses_its_digest_without_rehashing() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        std::fs::write(&compiler, b"abc").unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", &compiler).unwrap(),
            vec![],
            Bytes::new(),
        );
        let local_digests = LocalDigestCache::default();
        let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let prepared = prepare_compilers(&invocation, 1024, &local_digests).unwrap();
        assert!(prepared.by_digest.contains_key(digest));

        // A cache hit must not read the file: a planted digest for the same fingerprint wins.
        let planted = "f".repeat(64);
        local_digests.0.lock().get_mut(&compiler).unwrap().1 = planted.clone();
        let prepared = prepare_compilers(&invocation, 1024, &local_digests).unwrap();
        assert!(prepared.by_digest.contains_key(&planted));

        std::fs::write(&compiler, b"abcd").unwrap();
        let prepared = prepare_compilers(&invocation, 1024, &local_digests).unwrap();
        assert!(prepared
            .by_digest
            .contains_key("88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589"));
    }

    #[test]
    fn seed_read_rejects_a_compiler_changed_after_hashing() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        std::fs::write(&compiler, b"abc").unwrap();
        let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

        assert_eq!(
            read_compiler_for_seed(&compiler, digest, 3).unwrap(),
            Bytes::from_static(b"abc")
        );
        assert!(matches!(
            read_compiler_for_seed(&compiler, digest, 2),
            Err(ExecutionError::UploadLimitExceeded { limit: 2 })
        ));
        std::fs::write(&compiler, b"abd").unwrap();
        assert!(matches!(
            read_compiler_for_seed(&compiler, digest, 3),
            Err(ExecutionError::Infrastructure(_))
        ));
    }

    #[test]
    fn executable_bytes_remain_part_of_the_logical_upload_limit() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        std::fs::write(&compiler, b"12345").unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::from_static(b"67890"),
        );

        assert!(matches!(
            prepare_compilers(&invocation, 9, &LocalDigestCache::default()),
            Err(ExecutionError::UploadLimitExceeded { limit: 9 })
        ));
        assert!(prepare_compilers(&invocation, 10, &LocalDigestCache::default()).is_ok());
    }

    #[test]
    fn distinct_zksolc_and_solc_get_distinct_cache_paths() {
        let source_dir = tempfile::tempdir().unwrap();
        let zksolc = source_dir.path().join("zksolc");
        let solc = source_dir.path().join("solc");
        std::fs::write(&zksolc, b"zksolc binary").unwrap();
        std::fs::write(&solc, b"solc binary").unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("zksolc", "bin/zksolc", zksolc).unwrap(),
            vec![CommandArgument::prefixed_file("--solc=", "solc")],
            Bytes::new(),
        )
        .with_file(JobFile::executable("solc", "bin/solc", solc).unwrap());

        let compilers = prepare_compilers(&invocation, 1024, &LocalDigestCache::default()).unwrap();
        assert_eq!(compilers.by_digest.len(), 2);
        let zksolc_path = compilers.path_overrides.get("zksolc").unwrap();
        let solc_path = compilers.path_overrides.get("solc").unwrap();
        assert_ne!(zksolc_path, solc_path);
        assert_eq!(
            invocation
                .program_path_with_overrides(Path::new(JOB_ROOT), &compilers.path_overrides)
                .unwrap(),
            *zksolc_path
        );
        assert_eq!(
            invocation
                .resolved_args_with_overrides(Path::new(JOB_ROOT), &compilers.path_overrides)
                .unwrap(),
            vec![format!("--solc={}", solc_path.display())]
        );
    }

    #[test]
    fn cache_volume_metadata_is_fail_closed() {
        let digest = "0".repeat(64);
        let legacy = Volume {
            name: compiler_cache_volume_name(&digest),
            driver: "local".to_string(),
            labels: compiler_cache_labels(&digest),
            ..Default::default()
        };
        assert!(
            validate_cache_volume(&legacy, &digest).is_err(),
            "volumes without a generation have no identity to bind warm-cache state to"
        );

        let generation = Uuid::new_v4().to_string();
        let valid = Volume {
            labels: {
                let mut labels = compiler_cache_labels(&digest);
                labels.insert(
                    COMPILER_CACHE_GENERATION_LABEL.to_string(),
                    generation.clone(),
                );
                labels
            },
            ..legacy
        };
        assert_eq!(validate_cache_volume(&valid, &digest).unwrap(), generation);

        let mut wrong_labels = valid.clone();
        wrong_labels.labels.clear();
        assert!(validate_cache_volume(&wrong_labels, &digest).is_err());
        let mut empty_generation = valid.clone();
        empty_generation
            .labels
            .insert(COMPILER_CACHE_GENERATION_LABEL.to_string(), String::new());
        assert!(validate_cache_volume(&empty_generation, &digest).is_err());
        let mut invalid_generation = valid.clone();
        invalid_generation.labels.insert(
            COMPILER_CACHE_GENERATION_LABEL.to_string(),
            "not-a-uuid".to_string(),
        );
        assert!(validate_cache_volume(&invalid_generation, &digest).is_err());
        let mut extra_label = valid.clone();
        extra_label
            .labels
            .insert("unexpected".to_string(), "value".to_string());
        assert!(validate_cache_volume(&extra_label, &digest).is_err());
        let mut driver_options = valid.clone();
        driver_options
            .options
            .insert("unexpected".to_string(), "value".to_string());
        assert!(validate_cache_volume(&driver_options, &digest).is_err());
        let mut wrong_driver = valid;
        wrong_driver.driver = "nfs".to_string();
        assert!(validate_cache_volume(&wrong_driver, &digest).is_err());
    }

    #[test]
    fn cache_seed_archive_has_a_single_normalized_read_only_file() {
        let archive = build_compiler_seed_archive(Bytes::from_static(b"compiler"), 2048).unwrap();
        assert_eq!(archive.len(), 2048);
        let mut archive = tar::Archive::new(archive.file);
        let entries = archive
            .entries()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path().unwrap().as_ref(), Path::new(".incoming"));
        assert_eq!(entries[0].header().mode().unwrap(), 0o444);
        assert_eq!(entries[0].header().uid().unwrap(), 0);
        assert_eq!(entries[0].header().gid().unwrap(), 0);
        assert!(matches!(
            build_compiler_seed_archive(Bytes::from_static(b"compiler"), 2047),
            Err(ExecutionError::UploadLimitExceeded { limit: 2047 })
        ));
    }

    #[tokio::test]
    async fn spooled_archive_stream_uses_bounded_chunks() {
        let archive =
            build_compiler_seed_archive(Bytes::from(vec![0x5a; 128 * 1024]), 256 * 1024).unwrap();
        let expected_len = archive.len();
        let mut stream = archive.into_stream();
        let mut streamed_len = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.unwrap();
            assert!(chunk.len() <= 64 * 1024);
            streamed_len += chunk.len();
        }
        assert_eq!(streamed_len, expected_len);
    }

    #[test]
    fn physical_archive_limit_includes_stdin() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        let source = source_dir.path().join("input.sol");
        std::fs::write(&compiler, b"binary").unwrap();
        std::fs::write(&source, b"x").unwrap();

        let invocation_without_stdin = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", &compiler).unwrap(),
            vec![],
            Bytes::new(),
        )
        .with_file(JobFile::regular_file("input", "input.sol", &source).unwrap());
        let compilers = prepare_compilers(
            &invocation_without_stdin,
            2048,
            &LocalDigestCache::default(),
        )
        .unwrap();
        let archive =
            build_archive(&invocation_without_stdin, 2048, compilers.logical_bytes).unwrap();
        assert_eq!(archive.len(), 2048);

        let invocation_with_stdin = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::from_static(b"x"),
        )
        .with_file(JobFile::regular_file("input", "input.sol", source).unwrap());
        let compilers =
            prepare_compilers(&invocation_with_stdin, 2048, &LocalDigestCache::default()).unwrap();
        assert!(matches!(
            build_archive(&invocation_with_stdin, 2048, compilers.logical_bytes),
            Err(ExecutionError::UploadLimitExceeded { limit: 2048 })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn archive_rejects_symlink_sources() {
        use std::os::unix::fs::symlink;

        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler-link");
        symlink("/bin/true", &compiler).unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::new(),
        );

        assert!(prepare_compilers(&invocation, 1024 * 1024, &LocalDigestCache::default()).is_err());
    }

    #[tokio::test]
    #[ignore = "requires SCV_TEST_LOCAL_DOCKER_RUNNER_IMAGE"]
    async fn local_docker_cache_round_trip() {
        let docker = Docker::connect_with_defaults().unwrap();
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        let marker = Uuid::new_v4();
        tokio::fs::write(
            &compiler,
            format!(
                "#!/bin/sh\n# {marker}\nif echo poison >> \"$0\" 2>/tmp/cache-write-error; then exit 70; fi\n/bin/cat\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::from_static(b"cached compiler input"),
        );
        let prepared =
            prepare_compilers(&invocation, 1024 * 1024, &LocalDigestCache::default()).unwrap();
        let digest = prepared.by_digest.keys().next().unwrap().clone();
        let volume_name = compiler_cache_volume_name(&digest);

        let mut settings = settings();
        settings.runner_image = std::env::var("SCV_TEST_LOCAL_DOCKER_RUNNER_IMAGE").unwrap();
        settings.platform =
            std::env::var("SCV_TEST_DOCKER_PLATFORM").unwrap_or_else(|_| "linux/amd64".to_string());
        let settings = Arc::new(settings);
        let new_executor = || DockerCompilerExecutor {
            docker: docker.clone(),
            settings: settings.clone(),
            // A separate state models independent verifier pods sharing one Docker host.
            compiler_cache: Arc::new(CompilerCacheState::default()),
            initialization: Arc::new(OnceCell::new_with(Some(Arc::new(ContainerJanitor::spawn(
                docker.clone(),
            ))))),
        };

        let first = new_executor();
        let second = new_executor();
        let (first_output, second_output) = tokio::join!(
            first.execute(invocation.clone()),
            second.execute(invocation.clone())
        );
        for output in [first_output.unwrap(), second_output.unwrap()] {
            output.ensure_success("test compiler").unwrap();
            assert_eq!(output.stdout, Bytes::from_static(b"cached compiler input"));
        }
        let warm = new_executor();
        let warm_output = warm.execute(invocation.clone()).await.unwrap();
        warm_output.ensure_success("test compiler").unwrap();
        assert_eq!(
            warm_output.stdout,
            Bytes::from_static(b"cached compiler input")
        );
        let repeated_warm_output = warm.execute(invocation.clone()).await.unwrap();
        repeated_warm_output
            .ensure_success("test compiler")
            .unwrap();
        assert_eq!(
            repeated_warm_output.stdout,
            Bytes::from_static(b"cached compiler input")
        );
        let volume = docker.inspect_volume(&volume_name).await.unwrap();
        let original_generation = validate_cache_volume(&volume, &digest).unwrap();
        docker
            .remove_volume(
                &volume_name,
                Some(
                    bollard::query_parameters::RemoveVolumeOptionsBuilder::default()
                        .force(true)
                        .build(),
                ),
            )
            .await
            .unwrap();

        let recovered_output = warm.execute(invocation.clone()).await.unwrap();
        recovered_output.ensure_success("test compiler").unwrap();
        assert_eq!(
            recovered_output.stdout,
            Bytes::from_static(b"cached compiler input")
        );
        let replacement = docker.inspect_volume(&volume_name).await.unwrap();
        let replacement_generation = validate_cache_volume(&replacement, &digest).unwrap();
        assert_ne!(original_generation, replacement_generation);

        let restarted_output = new_executor().execute(invocation.clone()).await.unwrap();
        restarted_output.ensure_success("test compiler").unwrap();
        assert_eq!(
            restarted_output.stdout,
            Bytes::from_static(b"cached compiler input")
        );
        docker
            .remove_volume(
                &volume_name,
                Some(
                    bollard::query_parameters::RemoveVolumeOptionsBuilder::default()
                        .force(true)
                        .build(),
                ),
            )
            .await
            .unwrap();

        // Unlabeled volumes and volumes without a generation are both rejected, not adopted.
        for labels in [HashMap::new(), compiler_cache_labels(&digest)] {
            docker
                .create_volume(VolumeCreateRequest {
                    name: Some(volume_name.clone()),
                    driver: Some("local".to_string()),
                    labels: Some(labels.clone()),
                    ..Default::default()
                })
                .await
                .unwrap();
            let error = new_executor()
                .execute(invocation.clone())
                .await
                .unwrap_err();
            assert!(matches!(error, ExecutionError::Infrastructure(_)));
            let foreign_volume = docker.inspect_volume(&volume_name).await.unwrap();
            assert_eq!(foreign_volume.labels, labels);
            docker
                .remove_volume(
                    &volume_name,
                    Some(
                        bollard::query_parameters::RemoveVolumeOptionsBuilder::default()
                            .force(true)
                            .build(),
                    ),
                )
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "requires SCV_TEST_LOCAL_DOCKER_RUNNER_IMAGE"]
    async fn local_docker_cache_phase_deadline_covers_digest_lock_wait() {
        let docker = Docker::connect_with_defaults().unwrap();
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        tokio::fs::write(
            &compiler,
            format!("#!/bin/sh\n# {}\n/bin/cat\n", Uuid::new_v4()).as_bytes(),
        )
        .await
        .unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::new(),
        );

        let mut settings = settings();
        settings.runner_image = std::env::var("SCV_TEST_LOCAL_DOCKER_RUNNER_IMAGE").unwrap();
        settings.platform =
            std::env::var("SCV_TEST_DOCKER_PLATFORM").unwrap_or_else(|_| "linux/amd64".to_string());
        settings.api_timeout_seconds = 2;
        settings.execution_timeout_seconds = 2;
        let cache_timeout_seconds = settings.cache_phase_timeout_seconds(1);
        let executor = DockerCompilerExecutor {
            docker: docker.clone(),
            settings: Arc::new(settings),
            compiler_cache: Arc::new(CompilerCacheState::default()),
            initialization: Arc::new(OnceCell::new_with(Some(Arc::new(ContainerJanitor::spawn(
                docker.clone(),
            ))))),
        };
        let digest = prepare_compilers(&invocation, 1024 * 1024, &LocalDigestCache::default())
            .unwrap()
            .by_digest
            .into_keys()
            .next()
            .unwrap();

        // Another in-process fill of the same digest that never finishes.
        let digest_lock = Arc::new(Mutex::new(()));
        executor
            .compiler_cache
            .locks
            .lock()
            .await
            .insert(digest.clone(), digest_lock.clone());
        let _held = digest_lock.lock().await;

        let error = timeout(
            Duration::from_secs(cache_timeout_seconds + 30),
            executor.execute(invocation),
        )
        .await
        .expect("the cache phase must not wait for the digest lock indefinitely")
        .unwrap_err();
        assert!(
            matches!(error, ExecutionError::Timeout { seconds } if seconds == cache_timeout_seconds)
        );

        let filters = HashMap::from([
            ("label".to_string(), vec![JOB_LABEL.to_string()]),
            (
                "volume".to_string(),
                vec![compiler_cache_volume_name(&digest)],
            ),
        ]);
        let leftover = docker
            .list_containers(Some(
                ListContainersOptionsBuilder::default()
                    .all(true)
                    .filters(&filters)
                    .build(),
            ))
            .await
            .unwrap();
        assert!(
            leftover.is_empty(),
            "timed-out job container must be removed"
        );
        docker
            .remove_volume(
                &compiler_cache_volume_name(&digest),
                Some(
                    bollard::query_parameters::RemoveVolumeOptionsBuilder::default()
                        .force(true)
                        .build(),
                ),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires SCV_TEST_DOCKER_ADDR and SCV_TEST_DOCKER_RUNNER_IMAGE"]
    async fn ssh_docker_round_trip() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        tokio::fs::write(&compiler, b"#!/bin/sh\n/bin/cat\n")
            .await
            .unwrap();
        let mut settings = settings();
        settings.addr = std::env::var("SCV_TEST_DOCKER_ADDR").unwrap();
        settings.key_path = std::env::var_os("SCV_TEST_DOCKER_KEY_PATH").map(PathBuf::from);
        settings.runner_image = std::env::var("SCV_TEST_DOCKER_RUNNER_IMAGE").unwrap();
        if let Ok(platform) = std::env::var("SCV_TEST_DOCKER_PLATFORM") {
            settings.platform = platform;
        }
        let executor = DockerCompilerExecutor::connect(settings).await.unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::from_static(b"remote compiler input"),
        );

        for _ in 0..2 {
            let output = executor.execute(invocation.clone()).await.unwrap();
            output.ensure_success("test compiler").unwrap();
            assert_eq!(output.stdout, Bytes::from_static(b"remote compiler input"));
            assert!(output.stderr.is_empty());
        }
    }

    #[tokio::test]
    #[ignore = "requires remote Docker test settings and network access"]
    async fn ssh_docker_vyper_round_trip() {
        let mut settings = settings();
        settings.addr = std::env::var("SCV_TEST_DOCKER_ADDR").unwrap();
        settings.key_path = std::env::var_os("SCV_TEST_DOCKER_KEY_PATH").map(PathBuf::from);
        settings.runner_image = std::env::var("SCV_TEST_DOCKER_RUNNER_IMAGE").unwrap();
        if let Ok(platform) = std::env::var("SCV_TEST_DOCKER_PLATFORM") {
            settings.platform = platform;
        }
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
}
