// SPDX-License-Identifier: LicenseRef-Blockscout

//! Runs [`SandboxJob`]s on the remote Docker host.
//!
//! A job's volumes are in-memory and vanish once no container has them mounted, so a holder
//! container keeps them mounted from before the first step until the outputs have been read.

use super::{
    force_remove_container, retry_connect, unix_timestamp, ContainerLimits, DockerCompilerExecutor,
    RunLimits, CLEANUP_GRACE_SECONDS, CONTAINER_FAMILY_LABEL, HOME_DIR, HOME_TMPFS_SIZE_BYTES,
    JOB_EXPIRY_LABEL, JOB_LABEL, JOB_USER,
};
use crate::{
    metrics::{self, ContainerFamily},
    ExecutionError, SandboxFile, SandboxJob, SandboxJobOutput, SandboxOutput, SandboxStep,
    SandboxVolume,
};
use anyhow::Context;
use bollard::{
    models::{
        ContainerCreateBody, HostConfig, Mount, MountType, MountVolumeOptions, VolumeCreateRequest,
    },
    query_parameters::{
        CreateContainerOptionsBuilder, DownloadFromContainerOptionsBuilder,
        ListVolumesOptionsBuilder, RemoveVolumeOptionsBuilder,
    },
    Docker,
};
use bytes::Bytes;
use futures::StreamExt;
use std::{collections::HashMap, io::Read};
use uuid::Uuid;

const OBJECT_PREFIX: &str = "scv-sandbox-";
/// Where the holder container mounts the job's volumes, one directory per volume.
const HOLDER_ROOT: &str = "/volumes";
const HOLDER_LIMITS: ContainerLimits = ContainerLimits {
    memory_bytes: 32 * 1024 * 1024,
    nano_cpus: 100_000_000,
    pids_limit: 16,
};

impl DockerCompilerExecutor {
    /// Runs the job's steps in order and returns their output and the requested files.
    ///
    /// A step that exits with a non-zero code ends the job with
    /// [`ExecutionError::CompilerFailed`], naming the step.
    pub async fn run_sandbox_job(
        &self,
        job: SandboxJob,
    ) -> Result<SandboxJobOutput, ExecutionError> {
        let observation = metrics::JobObservation::new(metrics::DOCKER, "sandbox_job");
        let result = async {
            job.validate()?;
            self.ensure_initialized().await?;
            let plan = JobPlan::new(&job, self.settings.request_timeout_seconds())?;
            let mut resources = SandboxResources::new(self.docker.clone());
            let result = self.run_planned_job(&job, &plan, &mut resources).await;
            if let Err(error) = resources.remove_all().await {
                tracing::warn!(
                    job_id = %plan.job_id,
                    error = ?error,
                    "eager sandbox cleanup failed; the periodic sweep remains armed"
                );
            }
            result
        }
        .await;
        observation.finish_result(&result);
        result
    }

    async fn run_planned_job(
        &self,
        job: &SandboxJob,
        plan: &JobPlan,
        resources: &mut SandboxResources,
    ) -> Result<SandboxJobOutput, ExecutionError> {
        for volume in &job.volumes {
            let request = plan.volume_request(volume);
            let name = plan.volume_name(&volume.name);
            retry_connect(&self.docker, || self.docker.create_volume(request.clone()))
                .await
                .context("create sandbox volume")
                .map_err(ExecutionError::Infrastructure)?;
            resources.volumes.push(name);
        }

        let holder_id = self
            .create_sandbox_container(
                plan.container_name("holder"),
                self.holder_config(job, plan),
                ContainerFamily::SandboxHolder,
                resources,
            )
            .await?;
        retry_connect(&self.docker, || {
            self.docker.start_container(&holder_id, None)
        })
        .await
        .context("start sandbox holder")
        .map_err(ExecutionError::Infrastructure)?;

        let mut steps = Vec::with_capacity(job.steps.len());
        for (index, step) in job.steps.iter().enumerate() {
            let container_id = self
                .create_sandbox_container(
                    plan.container_name(&format!("step-{index}")),
                    self.step_config(step, plan),
                    ContainerFamily::SandboxStep,
                    resources,
                )
                .await?;
            let output = self
                .run_created_container_within(
                    &container_id,
                    Bytes::new(),
                    ContainerFamily::SandboxStep,
                    RunLimits {
                        timeout_seconds: step.limits.timeout_seconds,
                        max_output_bytes: step.limits.max_output_bytes,
                    },
                )
                .await?;
            // A finished step must not keep its mounts or memory while the next one runs.
            if let Err(error) = resources.remove_container(&container_id).await {
                tracing::warn!(container_id, error = ?error, "failed to remove sandbox step");
            }
            output.ensure_success(&step.name)?;
            steps.push(output);
        }

        let mut outputs = Vec::with_capacity(job.outputs.len());
        for output in &job.outputs {
            outputs.push(self.read_sandbox_output(&holder_id, output).await?);
        }
        Ok(SandboxJobOutput { steps, outputs })
    }

    async fn create_sandbox_container(
        &self,
        name: String,
        config: ContainerCreateBody,
        family: ContainerFamily,
        resources: &mut SandboxResources,
    ) -> Result<String, ExecutionError> {
        let _timer = metrics::start_operation(metrics::DOCKER, metrics::CREATE, family.as_str());
        let created = retry_connect(&self.docker, || {
            self.docker.create_container(
                Some(
                    CreateContainerOptionsBuilder::default()
                        .name(&name)
                        .platform(&self.settings.platform)
                        .build(),
                ),
                config.clone(),
            )
        })
        .await
        .context("create sandbox container")
        .map_err(ExecutionError::Infrastructure)?;
        resources
            .containers
            .push((created.id.clone(), metrics::observe_container(family)));
        Ok(created.id)
    }

    /// Keeps every volume mounted for the whole job and is where outputs are read from. It runs
    /// nothing from the job.
    fn holder_config(&self, job: &SandboxJob, plan: &JobPlan) -> ContainerCreateBody {
        let mounts = job
            .volumes
            .iter()
            .map(|volume| plan.mount(&volume.name, format!("{HOLDER_ROOT}/{}", volume.name), true))
            .collect();
        ContainerCreateBody {
            attach_stdin: Some(false),
            attach_stdout: Some(false),
            attach_stderr: Some(false),
            open_stdin: Some(false),
            stdin_once: Some(false),
            ..self.locked_down_container(
                JOB_USER,
                Vec::new(),
                vec![
                    "/usr/bin/sleep".to_string(),
                    plan.lifetime_seconds.to_string(),
                ],
                "/",
                plan.labels(ContainerFamily::SandboxHolder),
                self.hardened_host_config(HOLDER_LIMITS, 64, mounts),
            )
        }
    }

    fn step_config(&self, step: &SandboxStep, plan: &JobPlan) -> ContainerCreateBody {
        let mounts = step
            .mounts
            .iter()
            .map(|mount| plan.mount(&mount.volume, mount.path.clone(), mount.read_only))
            .collect();
        let limits = ContainerLimits {
            memory_bytes: i64::try_from(step.limits.memory_bytes).unwrap_or(i64::MAX),
            nano_cpus: i64::try_from(step.limits.nano_cpus).unwrap_or(i64::MAX),
            pids_limit: i64::try_from(step.limits.pids_limit).unwrap_or(i64::MAX),
        };
        let host_config = HostConfig {
            tmpfs: Some(HashMap::from([(
                HOME_DIR.to_string(),
                format!("rw,noexec,nosuid,nodev,size={HOME_TMPFS_SIZE_BYTES}"),
            )])),
            ..self.hardened_host_config(limits, 1024, mounts)
        };
        let mut env = Vec::with_capacity(step.env.len() + 1);
        if !step.env.iter().any(|(name, _)| name == "HOME") {
            env.push(format!("HOME={HOME_DIR}"));
        }
        env.extend(
            step.env
                .iter()
                .map(|(name, value)| format!("{name}={value}")),
        );
        ContainerCreateBody {
            image: Some(step.image.clone()),
            ..self.locked_down_container(
                JOB_USER,
                env,
                step.command.clone(),
                &step.working_dir,
                plan.labels(ContainerFamily::SandboxStep),
                host_config,
            )
        }
    }

    async fn read_sandbox_output(
        &self,
        holder_id: &str,
        output: &SandboxOutput,
    ) -> Result<Vec<SandboxFile>, ExecutionError> {
        let path = format!("{HOLDER_ROOT}/{}/{}", output.volume, output.path);
        let options = DownloadFromContainerOptionsBuilder::default()
            .path(&path)
            .build();
        let archive = retry_connect(&self.docker, || async {
            let mut stream = self
                .docker
                .download_from_container(holder_id, Some(options.clone()));
            let mut archive = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                if archive.len() + chunk.len() > output.max_bytes {
                    return Ok(None);
                }
                archive.extend_from_slice(&chunk);
            }
            Ok(Some(archive))
        })
        .await;
        let archive = match archive {
            Ok(Some(archive)) => archive,
            Ok(None) => {
                return Err(ExecutionError::OutputLimitExceeded {
                    limit: output.max_bytes,
                })
            }
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => {
                return Err(ExecutionError::Infrastructure(anyhow::anyhow!(
                    "sandbox output {} was not produced",
                    output.name
                )))
            }
            Err(error) => {
                return Err(ExecutionError::Infrastructure(
                    anyhow::Error::new(error).context("read sandbox output"),
                ))
            }
        };
        metrics::add_transfer_bytes(
            metrics::DOCKER,
            metrics::OUTPUT,
            "sandbox_job",
            archive.len(),
        );
        regular_files(&archive)
            .with_context(|| format!("unpack sandbox output {}", output.name))
            .map_err(ExecutionError::Infrastructure)
    }
}

/// Names, labels and lifetime shared by everything one job creates on the Docker host.
struct JobPlan {
    job_id: Uuid,
    lifetime_seconds: u64,
    expiry: u64,
}

impl JobPlan {
    fn new(job: &SandboxJob, request_timeout_seconds: u64) -> Result<Self, ExecutionError> {
        let lifetime_seconds = job_lifetime_seconds(job, request_timeout_seconds);
        Ok(Self {
            job_id: Uuid::new_v4(),
            lifetime_seconds,
            expiry: unix_timestamp()?
                .saturating_add(lifetime_seconds)
                .saturating_add(CLEANUP_GRACE_SECONDS),
        })
    }

    fn volume_name(&self, volume: &str) -> String {
        format!("{OBJECT_PREFIX}{}-{volume}", self.job_id)
    }

    fn container_name(&self, role: &str) -> String {
        format!("{OBJECT_PREFIX}{}-{role}", self.job_id)
    }

    /// The expiry lets the periodic sweep reclaim whatever a dead verifier process left behind.
    fn base_labels(&self) -> HashMap<String, String> {
        HashMap::from([
            (JOB_LABEL.to_string(), "true".to_string()),
            (JOB_EXPIRY_LABEL.to_string(), self.expiry.to_string()),
        ])
    }

    fn labels(&self, family: ContainerFamily) -> HashMap<String, String> {
        let mut labels = self.base_labels();
        labels.insert(
            CONTAINER_FAMILY_LABEL.to_string(),
            family.as_str().to_string(),
        );
        labels
    }

    fn volume_request(&self, volume: &SandboxVolume) -> VolumeCreateRequest {
        let (uid, gid) = JOB_USER.split_once(':').unwrap_or((JOB_USER, JOB_USER));
        VolumeCreateRequest {
            name: Some(self.volume_name(&volume.name)),
            driver: Some("local".to_string()),
            driver_opts: Some(HashMap::from([
                ("type".to_string(), "tmpfs".to_string()),
                ("device".to_string(), "tmpfs".to_string()),
                (
                    "o".to_string(),
                    format!(
                        "size={},uid={uid},gid={gid},mode=0700,nosuid,nodev",
                        volume.size_bytes
                    ),
                ),
            ])),
            labels: Some(self.base_labels()),
            ..Default::default()
        }
    }

    fn mount(&self, volume: &str, target: String, read_only: bool) -> Mount {
        Mount {
            target: Some(target),
            source: Some(self.volume_name(volume)),
            typ: Some(MountType::VOLUME),
            read_only: Some(read_only),
            volume_options: Some(MountVolumeOptions {
                no_copy: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}

/// Upper bound on how long a live owner needs the job's containers and volumes: every Docker
/// request it makes, plus each step's own run time.
fn job_lifetime_seconds(job: &SandboxJob, request_timeout_seconds: u64) -> u64 {
    let count = |items: usize| u64::try_from(items).unwrap_or(u64::MAX);
    // Create per volume; create and start the holder; create, attach, start, inspect and remove
    // per step; one download per output.
    let requests = count(job.volumes.len())
        .saturating_add(2)
        .saturating_add(count(job.steps.len()).saturating_mul(5))
        .saturating_add(count(job.outputs.len()));
    job.steps.iter().fold(
        requests.saturating_mul(request_timeout_seconds),
        |total, step| total.saturating_add(step.limits.timeout_seconds),
    )
}

/// The regular files of a tar archive. Links, devices and directories are dropped: the archive
/// comes from a volume that untrusted steps wrote.
fn regular_files(archive: &[u8]) -> anyhow::Result<Vec<SandboxFile>> {
    let mut files = Vec::new();
    for entry in tar::Archive::new(archive).entries()? {
        let mut entry = entry?;
        if entry.header().entry_type() != tar::EntryType::Regular {
            continue;
        }
        let path = entry.path()?.to_string_lossy().into_owned();
        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;
        files.push(SandboxFile {
            path,
            content: Bytes::from(content),
        });
    }
    Ok(files)
}

/// Everything a job created on the Docker host, removed in an order that works: containers
/// first, because a mounted volume cannot be removed.
struct SandboxResources {
    docker: Docker,
    containers: Vec<(String, metrics::GaugeGuard)>,
    volumes: Vec<String>,
}

impl SandboxResources {
    fn new(docker: Docker) -> Self {
        Self {
            docker,
            containers: Vec::new(),
            volumes: Vec::new(),
        }
    }

    async fn remove_container(&mut self, container_id: &str) -> anyhow::Result<()> {
        let _timer = metrics::start_operation(
            metrics::DOCKER,
            metrics::CLEANUP,
            ContainerFamily::SandboxStep.as_str(),
        );
        force_remove_container(&self.docker, container_id).await?;
        self.containers.retain(|(id, _)| id != container_id);
        Ok(())
    }

    async fn remove_all(&mut self) -> anyhow::Result<()> {
        let containers = std::mem::take(&mut self.containers);
        let volumes = std::mem::take(&mut self.volumes);
        remove_resources(&self.docker, containers, volumes).await
    }
}

impl Drop for SandboxResources {
    /// Reached with resources left only when the job's future was dropped mid-run.
    fn drop(&mut self) {
        if self.containers.is_empty() && self.volumes.is_empty() {
            return;
        }
        let docker = self.docker.clone();
        let containers = std::mem::take(&mut self.containers);
        let volumes = std::mem::take(&mut self.volumes);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Err(error) = remove_resources(&docker, containers, volumes).await {
                    tracing::error!(error = ?error, "failed to clean up a canceled sandbox job");
                }
            });
        } else {
            tracing::error!("could not schedule sandbox cleanup without a Tokio runtime");
        }
    }
}

async fn remove_resources(
    docker: &Docker,
    containers: Vec<(String, metrics::GaugeGuard)>,
    volumes: Vec<String>,
) -> anyhow::Result<()> {
    let mut first_error = None;
    // Steps before the holder they share volumes with.
    for (container_id, _active) in containers.into_iter().rev() {
        if let Err(error) = force_remove_container(docker, &container_id).await {
            first_error.get_or_insert(error.context(format!("remove container {container_id}")));
        }
    }
    for volume in volumes {
        if let Err(error) = remove_volume(docker, &volume).await {
            first_error.get_or_insert(error.context(format!("remove volume {volume}")));
        }
    }
    first_error.map_or(Ok(()), Err)
}

async fn remove_volume(docker: &Docker, name: &str) -> anyhow::Result<()> {
    let options = RemoveVolumeOptionsBuilder::default().build();
    let removed = retry_connect(docker, || docker.remove_volume(name, Some(options.clone()))).await;
    match removed {
        Ok(())
        | Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Removes expired sandbox volumes whose owner did not. Runs after the container sweep, since a
/// volume can only go once nothing mounts it.
pub(super) async fn reap_expired_volumes(docker: &Docker) -> anyhow::Result<()> {
    let filters = HashMap::from([("label".to_string(), vec![JOB_LABEL.to_string()])]);
    let options = ListVolumesOptionsBuilder::default()
        .filters(&filters)
        .build();
    let now = unix_timestamp()?;
    let listed = retry_connect(docker, || docker.list_volumes(Some(options.clone()))).await?;
    let mut first_error = None;
    for volume in listed.volumes.unwrap_or_default() {
        // Compiler jobs label their anonymous volume too, and Docker removes that with its
        // container; only volumes this module named are reclaimed here.
        if !volume.name.starts_with(OBJECT_PREFIX) {
            continue;
        }
        let expired = volume
            .labels
            .get(JOB_EXPIRY_LABEL)
            .and_then(|expiry| expiry.parse::<u64>().ok())
            .is_some_and(|expiry| expiry <= now);
        if !expired {
            continue;
        }
        match remove_volume(docker, &volume.name).await {
            Ok(()) => tracing::warn!(volume = volume.name, "removed expired sandbox volume"),
            Err(error) => {
                tracing::error!(volume = volume.name, error = ?error, "failed to remove expired sandbox volume");
                first_error.get_or_insert(error.context(format!("remove volume {}", volume.name)));
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::{
        super::{tests::settings, CompilerCacheState, ContainerJanitor},
        *,
    };
    use crate::{
        sandbox::tests::{limits, pinned},
        SandboxMount,
    };
    use std::sync::Arc;
    use tokio::sync::OnceCell;

    fn executor() -> DockerCompilerExecutor {
        DockerCompilerExecutor::new(settings()).unwrap()
    }

    fn step(name: &str, script: &str, mounts: Vec<SandboxMount>) -> SandboxStep {
        SandboxStep {
            name: name.to_string(),
            image: pinned("tools"),
            command: vec!["/bin/sh".to_string(), "-c".to_string(), script.to_string()],
            env: vec![("MODE".to_string(), "test".to_string())],
            working_dir: "/".to_string(),
            mounts,
            limits: limits(),
        }
    }

    fn mount(volume: &str, path: &str, read_only: bool) -> SandboxMount {
        SandboxMount {
            volume: volume.to_string(),
            path: path.to_string(),
            read_only,
        }
    }

    fn volume(name: &str, size_bytes: u64) -> SandboxVolume {
        SandboxVolume {
            name: name.to_string(),
            size_bytes,
        }
    }

    fn two_volume_job() -> SandboxJob {
        SandboxJob {
            volumes: vec![volume("data", 8 << 20), volume("out", 8 << 20)],
            steps: vec![
                step("write", "true", vec![mount("data", "/data", false)]),
                step(
                    "read",
                    "true",
                    vec![
                        mount("data", "/data", true),
                        mount("out", "/data/nested", false),
                    ],
                ),
            ],
            outputs: Vec::new(),
        }
    }

    #[test]
    fn step_container_is_locked_down_and_uses_the_step_image() {
        let executor = executor();
        let job = two_volume_job();
        let plan = JobPlan::new(&job, 10).unwrap();
        let config = executor.step_config(&job.steps[1], &plan);

        assert_eq!(config.image.as_deref(), Some(pinned("tools").as_str()));
        assert_eq!(config.user.as_deref(), Some("65532:65532"));
        assert_eq!(config.network_disabled, Some(true));
        assert_eq!(
            config.env,
            Some(vec!["HOME=/tmp".to_string(), "MODE=test".to_string()])
        );
        let labels = config.labels.unwrap();
        assert_eq!(labels[CONTAINER_FAMILY_LABEL], "sandbox_step");
        assert_eq!(labels[JOB_EXPIRY_LABEL], plan.expiry.to_string());

        let host = config.host_config.unwrap();
        assert_eq!(host.network_mode.as_deref(), Some("none"));
        assert_eq!(host.readonly_rootfs, Some(true));
        assert_eq!(host.cap_drop, Some(vec!["ALL".to_string()]));
        assert_eq!(host.privileged, Some(false));
        assert_eq!(
            host.security_opt,
            Some(vec!["no-new-privileges:true".to_string()])
        );
        assert_eq!(host.memory, Some(256 * 1024 * 1024));
        assert_eq!(host.memory_swap, host.memory);
        assert_eq!(host.pids_limit, Some(64));
        assert!(host.tmpfs.unwrap()["/tmp"].contains("noexec"));

        let mounts: Vec<_> = host
            .mounts
            .unwrap()
            .into_iter()
            .map(|mount| {
                (
                    mount.source.unwrap(),
                    mount.target.unwrap(),
                    mount.read_only,
                )
            })
            .collect();
        assert_eq!(
            mounts,
            [
                (plan.volume_name("data"), "/data".to_string(), Some(true)),
                (
                    plan.volume_name("out"),
                    "/data/nested".to_string(),
                    Some(false)
                ),
            ]
        );
    }

    #[test]
    fn holder_mounts_every_volume_read_only_on_the_runner_image() {
        let executor = executor();
        let job = two_volume_job();
        let plan = JobPlan::new(&job, 10).unwrap();
        let config = executor.holder_config(&job, &plan);

        assert_eq!(config.image, Some(settings().runner_image));
        assert_eq!(config.attach_stdout, Some(false));
        assert_eq!(config.open_stdin, Some(false));
        assert_eq!(
            config.cmd,
            Some(vec![
                "/usr/bin/sleep".to_string(),
                plan.lifetime_seconds.to_string()
            ])
        );
        assert_eq!(
            config.labels.unwrap()[CONTAINER_FAMILY_LABEL],
            "sandbox_holder"
        );
        let targets: Vec<_> = config
            .host_config
            .unwrap()
            .mounts
            .unwrap()
            .into_iter()
            .map(|mount| (mount.target.unwrap(), mount.read_only))
            .collect();
        assert_eq!(
            targets,
            [
                ("/volumes/data".to_string(), Some(true)),
                ("/volumes/out".to_string(), Some(true)),
            ]
        );
    }

    #[test]
    fn volumes_are_capped_in_memory_and_owned_by_the_job_user() {
        let job = two_volume_job();
        let plan = JobPlan::new(&job, 10).unwrap();
        let request = plan.volume_request(&job.volumes[0]);

        assert_eq!(request.name, Some(plan.volume_name("data")));
        assert!(request.name.unwrap().starts_with(OBJECT_PREFIX));
        let options = request.driver_opts.unwrap();
        assert_eq!(options["type"], "tmpfs");
        assert_eq!(
            options["o"],
            "size=8388608,uid=65532,gid=65532,mode=0700,nosuid,nodev"
        );
        assert!(request.labels.unwrap().contains_key(JOB_EXPIRY_LABEL));
    }

    #[test]
    fn job_lifetime_covers_every_request_and_step() {
        let mut job = two_volume_job();
        job.outputs.push(SandboxOutput {
            name: "copy".to_string(),
            volume: "out".to_string(),
            path: "copy".to_string(),
            max_bytes: 1024,
        });
        // 2 volumes + 2 holder requests + 2 steps * 5 + 1 output = 15 requests of 10 s, plus two
        // 30-second steps.
        assert_eq!(job_lifetime_seconds(&job, 10), 15 * 10 + 2 * 30);
    }

    #[test]
    fn only_regular_files_are_read_from_an_output_archive() {
        let mut builder = tar::Builder::new(Vec::new());
        let mut file = tar::Header::new_gnu();
        file.set_size(5);
        file.set_mode(0o644);
        file.set_cksum();
        builder
            .append_data(&mut file, "dir/a", &b"hello"[..])
            .unwrap();
        let mut link = tar::Header::new_gnu();
        link.set_entry_type(tar::EntryType::Symlink);
        link.set_size(0);
        builder
            .append_link(&mut link, "dir/link", "/etc/passwd")
            .unwrap();
        let mut directory = tar::Header::new_gnu();
        directory.set_entry_type(tar::EntryType::Directory);
        directory.set_size(0);
        directory.set_cksum();
        builder
            .append_data(&mut directory, "dir/sub", std::io::empty())
            .unwrap();
        let archive = builder.into_inner().unwrap();

        assert_eq!(
            regular_files(&archive).unwrap(),
            [SandboxFile {
                path: "dir/a".to_string(),
                content: Bytes::from_static(b"hello"),
            }]
        );
    }

    fn local_executor(docker: &Docker, runner_image: &str) -> DockerCompilerExecutor {
        let mut settings = settings();
        settings.runner_image = runner_image.to_string();
        settings.platform =
            std::env::var("SCV_TEST_DOCKER_PLATFORM").unwrap_or_else(|_| "linux/amd64".to_string());
        DockerCompilerExecutor {
            docker: docker.clone(),
            settings: Arc::new(settings),
            compiler_cache: Arc::new(CompilerCacheState::default()),
            initialization: Arc::new(OnceCell::new_with(Some(Arc::new(ContainerJanitor::spawn(
                docker.clone(),
            ))))),
        }
    }

    async fn leftover_sandbox_objects(docker: &Docker) -> Vec<String> {
        let filters = HashMap::from([("name".to_string(), vec![OBJECT_PREFIX.to_string()])]);
        let containers = docker
            .list_containers(Some(
                bollard::query_parameters::ListContainersOptionsBuilder::default()
                    .all(true)
                    .filters(&filters)
                    .build(),
            ))
            .await
            .unwrap()
            .into_iter()
            .flat_map(|container| container.names.unwrap_or_default());
        let volumes = docker
            .list_volumes(Some(
                ListVolumesOptionsBuilder::default()
                    .filters(&filters)
                    .build(),
            ))
            .await
            .unwrap()
            .volumes
            .unwrap_or_default()
            .into_iter()
            .map(|volume| volume.name);
        containers.chain(volumes).collect()
    }

    #[tokio::test]
    #[ignore = "requires SCV_TEST_LOCAL_DOCKER_RUNNER_IMAGE"]
    async fn local_docker_sandbox_job_round_trip() {
        let docker = Docker::connect_with_defaults().unwrap();
        let image = std::env::var("SCV_TEST_LOCAL_DOCKER_RUNNER_IMAGE").unwrap();
        let executor = local_executor(&docker, &image);
        let step = |name: &str, script: &str, mounts| SandboxStep {
            image: image.clone(),
            ..step(name, script, mounts)
        };

        let job = SandboxJob {
            volumes: vec![volume("data", 8 << 20), volume("out", 8 << 20)],
            steps: vec![
                step(
                    "write",
                    "id -u && echo hello > /data/greeting && mkdir /data/dir /data/nested \
                     && echo a > /data/dir/a && ln -s /etc/passwd /data/dir/link",
                    vec![mount("data", "/data", false)],
                ),
                step(
                    "read",
                    // The source is read-only here, the root filesystem too, and the container
                    // has no network interface to the outside.
                    "if touch /data/x 2>/dev/null; then exit 3; fi; \
                     if touch /probe 2>/dev/null; then exit 4; fi; \
                     if [ -e /sys/class/net/eth0 ]; then exit 5; fi; \
                     cat /data/greeting > /data/nested/copy",
                    vec![
                        mount("data", "/data", true),
                        mount("out", "/data/nested", false),
                    ],
                ),
            ],
            outputs: vec![
                SandboxOutput {
                    name: "copy".to_string(),
                    volume: "out".to_string(),
                    path: "copy".to_string(),
                    max_bytes: 64 * 1024,
                },
                SandboxOutput {
                    name: "dir".to_string(),
                    volume: "data".to_string(),
                    path: "dir".to_string(),
                    max_bytes: 64 * 1024,
                },
            ],
        };
        let output = executor.run_sandbox_job(job.clone()).await.unwrap();
        assert_eq!(output.steps.len(), 2);
        assert_eq!(output.steps[0].stdout, Bytes::from_static(b"65532\n"));
        assert_eq!(
            output.outputs[0],
            [SandboxFile {
                path: "copy".to_string(),
                content: Bytes::from_static(b"hello\n"),
            }]
        );
        // The symlink the first step planted next to `a` is not returned.
        assert_eq!(
            output.outputs[1],
            [SandboxFile {
                path: "dir/a".to_string(),
                content: Bytes::from_static(b"a\n"),
            }]
        );

        // A failing step ends the job and names itself.
        let mut failing = job.clone();
        failing.steps[1] = step("fail", "echo boom >&2; exit 7", Vec::new());
        match executor.run_sandbox_job(failing).await {
            Err(ExecutionError::CompilerFailed {
                compiler,
                exit_code,
                stderr,
                ..
            }) => {
                assert_eq!(compiler, "fail");
                assert_eq!(exit_code, 7);
                assert_eq!(stderr, "boom\n");
            }
            other => panic!("unexpected result: {other:?}"),
        }

        // A volume cannot grow past its size.
        let mut overflowing = job.clone();
        overflowing.steps.truncate(1);
        overflowing.steps[0] = step(
            "fill",
            "head -c 16777216 /dev/zero > /data/big",
            vec![mount("data", "/data", false)],
        );
        overflowing.outputs.clear();
        assert!(matches!(
            executor.run_sandbox_job(overflowing).await,
            Err(ExecutionError::CompilerFailed { .. })
        ));

        // An output larger than its limit is refused rather than truncated.
        let mut oversized = job.clone();
        oversized.outputs[0].max_bytes = 16;
        assert!(matches!(
            executor.run_sandbox_job(oversized).await,
            Err(ExecutionError::OutputLimitExceeded { limit: 16 })
        ));

        assert_eq!(
            leftover_sandbox_objects(&docker).await,
            Vec::<String>::new()
        );
    }
}
