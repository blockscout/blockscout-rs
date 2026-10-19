// SPDX-License-Identifier: LicenseRef-Blockscout

//! Runs [`SandboxJob`]s on the remote Docker host.
//!
//! A job's volumes are in-memory and vanish once no container has them mounted, so a holder
//! container keeps them mounted from before the first step until the outputs have been read.
//!
//! Steps with egress join a per-job network that has no route out. The only other member is a
//! tinyproxy container, also attached to the default bridge, which tunnels `CONNECT` requests to
//! port 443 of the job's listed hosts and refuses everything else.

use super::{
    force_remove_container, retry_connect, unix_timestamp, ContainerLimits, DockerCompilerExecutor,
    RunLimits, CLEANUP_GRACE_SECONDS, CONTAINER_FAMILY_LABEL, HOME_DIR, HOME_TMPFS_SIZE_BYTES,
    JOB_EXPIRY_LABEL, JOB_LABEL, JOB_USER,
};
use crate::{
    metrics::{self, ContainerFamily},
    sandbox::validate_volumes,
    ExecutionError, ExecutionOutput, SandboxFile, SandboxJob, SandboxJobOutput, SandboxNetwork,
    SandboxOutput, SandboxSpec, SandboxStep, SandboxVolume,
};
use anyhow::Context;
use bollard::{
    exec::{CreateExecOptions, StartExecOptions, StartExecResults},
    models::{
        ContainerCreateBody, EndpointSettings, HostConfig, Mount, MountType, MountVolumeOptions,
        NetworkConnectRequest, NetworkCreateRequest, VolumeCreateRequest,
    },
    query_parameters::{
        CreateContainerOptionsBuilder, DownloadFromContainerOptionsBuilder,
        ListNetworksOptionsBuilder, ListVolumesOptionsBuilder, RemoveVolumeOptionsBuilder,
    },
    Docker,
};
use bytes::Bytes;
use futures::StreamExt;
use std::{collections::HashMap, io::Read, time::Duration};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

const OBJECT_PREFIX: &str = "scv-sandbox-";
/// Where the holder container mounts the job's volumes, one directory per volume.
const HOLDER_ROOT: &str = "/volumes";
const HOLDER_LIMITS: ContainerLimits = ContainerLimits {
    memory_bytes: 32 * 1024 * 1024,
    nano_cpus: 100_000_000,
    pids_limit: 16,
};
/// Name the proxy answers to on the job's egress network.
const PROXY_ALIAS: &str = "egress-proxy";
const PROXY_PORT: u16 = 3128;
const PROXY_LIMITS: ContainerLimits = ContainerLimits {
    memory_bytes: 64 * 1024 * 1024,
    nano_cpus: 500_000_000,
    pids_limit: 64,
};
const PROXY_CONFIG_PATH: &str = "/tmp/tinyproxy.conf";
const PROXY_FILTER_PATH: &str = "/tmp/tinyproxy-filter";
const PROXY_READY_POLL: Duration = Duration::from_millis(100);

/// A running sandbox: its volumes, their holder and, with egress hosts, its proxy. Steps are run
/// one at a time, so the caller can choose the next one from what the last one produced.
///
/// Call [`SandboxSession::finish`] when done. A session that is merely dropped is still cleaned
/// up, in the background.
pub struct SandboxSession {
    executor: DockerCompilerExecutor,
    spec: SandboxSpec,
    plan: JobPlan,
    resources: SandboxResources,
    holder_id: String,
    steps_run: usize,
    /// The last step's container until it is removed. A step that timed out may still be
    /// running, so nothing else starts in the session while this is set.
    unremoved_step: Option<String>,
    observation: Option<metrics::JobObservation>,
}

impl DockerCompilerExecutor {
    /// Runs the job's steps in order and returns their output and the requested files.
    ///
    /// A step that exits with a non-zero code ends the job with
    /// [`ExecutionError::CompilerFailed`], naming the step.
    pub async fn run_sandbox_job(
        &self,
        job: SandboxJob,
    ) -> Result<SandboxJobOutput, ExecutionError> {
        job.validate()?;
        let mut session = self
            .start_sandbox(SandboxSpec {
                volumes: job.volumes.clone(),
                egress_hosts: job.egress_hosts.clone(),
                lifetime_seconds: job_lifetime_seconds(
                    &job,
                    self.settings.request_timeout_seconds(),
                    self.settings.api_timeout_seconds,
                ),
            })
            .await?;
        let result = async {
            let mut steps = Vec::with_capacity(job.steps.len());
            for step in &job.steps {
                steps.push(session.run_step(step).await?);
            }
            let mut outputs = Vec::with_capacity(job.outputs.len());
            for output in &job.outputs {
                outputs.push(session.read_output(output).await?);
            }
            Ok(SandboxJobOutput { steps, outputs })
        }
        .await;
        session.finish().await;
        result
    }

    /// Creates the sandbox's volumes, their holder and, when egress hosts are listed, its proxy.
    pub async fn start_sandbox(&self, spec: SandboxSpec) -> Result<SandboxSession, ExecutionError> {
        let observation = metrics::JobObservation::new(metrics::DOCKER, "sandbox_job");
        let started = async {
            spec.validate()?;
            self.ensure_initialized().await?;
            let plan = JobPlan::new(spec.lifetime_seconds)?;
            let mut resources = SandboxResources::new(self.docker.clone());
            match self.provision_sandbox(&spec, &plan, &mut resources).await {
                Ok(holder_id) => Ok((plan, resources, holder_id)),
                Err(error) => {
                    resources.remove_all_logged(&plan).await;
                    Err(error)
                }
            }
        }
        .await;
        match started {
            Ok((plan, resources, holder_id)) => Ok(SandboxSession {
                executor: self.clone(),
                spec,
                plan,
                resources,
                holder_id,
                steps_run: 0,
                unremoved_step: None,
                observation: Some(observation),
            }),
            Err(error) => {
                observation.finish_error(&error);
                Err(error)
            }
        }
    }

    async fn provision_sandbox(
        &self,
        spec: &SandboxSpec,
        plan: &JobPlan,
        resources: &mut SandboxResources,
    ) -> Result<String, ExecutionError> {
        for volume in &spec.volumes {
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
                self.holder_config(&spec.volumes, plan),
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

        if !spec.egress_hosts.is_empty() {
            self.start_egress_proxy(&spec.egress_hosts, plan, resources)
                .await?;
        }
        Ok(holder_id)
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
    fn holder_config(&self, volumes: &[SandboxVolume], plan: &JobPlan) -> ContainerCreateBody {
        let mounts = volumes
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
        let egress = step.network == SandboxNetwork::Egress;
        let mut host_config = HostConfig {
            tmpfs: Some(HashMap::from([(
                HOME_DIR.to_string(),
                format!("rw,noexec,nosuid,nodev,size={HOME_TMPFS_SIZE_BYTES}"),
            )])),
            ..self.hardened_host_config(limits, 1024, mounts)
        };
        let mut env = Vec::with_capacity(step.env.len() + 5);
        if !step.env.iter().any(|(name, _)| name == "HOME") {
            env.push(format!("HOME={HOME_DIR}"));
        }
        if egress {
            host_config.network_mode = Some(plan.network_name());
            let proxy = format!("http://{PROXY_ALIAS}:{PROXY_PORT}");
            for name in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
                env.push(format!("{name}={proxy}"));
            }
        }
        env.extend(
            step.env
                .iter()
                .map(|(name, value)| format!("{name}={value}")),
        );
        ContainerCreateBody {
            image: Some(step.image.clone()),
            network_disabled: Some(!egress),
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

    /// Creates the job's egress network and its proxy, and waits until the proxy accepts
    /// connections, so the first step cannot race it.
    async fn start_egress_proxy(
        &self,
        hosts: &[String],
        plan: &JobPlan,
        resources: &mut SandboxResources,
    ) -> Result<(), ExecutionError> {
        let network = plan.network_name();
        let request = plan.network_request();
        retry_connect(&self.docker, || self.docker.create_network(request.clone()))
            .await
            .context("create sandbox egress network")
            .map_err(ExecutionError::Infrastructure)?;
        resources.networks.push(network.clone());

        let proxy_id = self
            .create_sandbox_container(
                plan.container_name("proxy"),
                self.proxy_config(hosts, plan),
                ContainerFamily::SandboxProxy,
                resources,
            )
            .await?;
        let connection = NetworkConnectRequest {
            container: proxy_id.clone(),
            endpoint_config: Some(EndpointSettings {
                aliases: Some(vec![PROXY_ALIAS.to_string()]),
                ..Default::default()
            }),
        };
        retry_connect(&self.docker, || {
            self.docker.connect_network(&network, connection.clone())
        })
        .await
        .context("connect sandbox egress proxy")
        .map_err(ExecutionError::Infrastructure)?;
        retry_connect(&self.docker, || {
            self.docker.start_container(&proxy_id, None)
        })
        .await
        .context("start sandbox egress proxy")
        .map_err(ExecutionError::Infrastructure)?;

        timeout(self.api_timeout(), async {
            while !self.proxy_accepts_connections(&proxy_id).await? {
                sleep(PROXY_READY_POLL).await;
            }
            Ok::<(), ExecutionError>(())
        })
        .await
        .map_err(|_| self.api_timeout_error("wait for the sandbox egress proxy"))?
    }

    /// Opens a TCP connection to the proxy from inside its own container.
    async fn proxy_accepts_connections(&self, proxy_id: &str) -> Result<bool, ExecutionError> {
        let probe = CreateExecOptions {
            cmd: Some(vec![
                "/bin/bash".to_string(),
                "-c".to_string(),
                format!("exec 3<>/dev/tcp/127.0.0.1/{PROXY_PORT}"),
            ]),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };
        let exec = retry_connect(&self.docker, || {
            self.docker.create_exec(proxy_id, probe.clone())
        })
        .await
        .context("create sandbox egress proxy probe")
        .map_err(ExecutionError::Infrastructure)?;
        let started = retry_connect(&self.docker, || {
            self.docker.start_exec(&exec.id, None::<StartExecOptions>)
        })
        .await
        .context("start sandbox egress proxy probe")
        .map_err(ExecutionError::Infrastructure)?;
        // The exit code is only final once the output stream has ended.
        if let StartExecResults::Attached { mut output, .. } = started {
            while output.next().await.is_some() {}
        }
        let inspected = retry_connect(&self.docker, || self.docker.inspect_exec(&exec.id))
            .await
            .context("inspect sandbox egress proxy probe")
            .map_err(ExecutionError::Infrastructure)?;
        Ok(inspected.exit_code == Some(0))
    }

    /// The only container of a job with a route out. It runs the runner image's tinyproxy with a
    /// configuration built here; nothing from the job reaches it but TCP connections.
    fn proxy_config(&self, hosts: &[String], plan: &JobPlan) -> ContainerCreateBody {
        let host_config = HostConfig {
            network_mode: Some("bridge".to_string()),
            tmpfs: Some(HashMap::from([(
                HOME_DIR.to_string(),
                format!("rw,noexec,nosuid,nodev,size={HOME_TMPFS_SIZE_BYTES}"),
            )])),
            ..self.hardened_host_config(PROXY_LIMITS, 1024, Vec::new())
        };
        ContainerCreateBody {
            attach_stdin: Some(false),
            attach_stdout: Some(false),
            attach_stderr: Some(false),
            open_stdin: Some(false),
            stdin_once: Some(false),
            network_disabled: Some(false),
            ..self.locked_down_container(
                JOB_USER,
                vec![
                    format!("TINYPROXY_CONF={}", proxy_configuration()),
                    format!("TINYPROXY_FILTER={}", proxy_filter(hosts)),
                ],
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    format!(
                        "printf '%s' \"$TINYPROXY_CONF\" > {PROXY_CONFIG_PATH} \
                         && printf '%s' \"$TINYPROXY_FILTER\" > {PROXY_FILTER_PATH} \
                         && exec /usr/bin/tinyproxy -d -c {PROXY_CONFIG_PATH}"
                    ),
                ],
                "/",
                plan.labels(ContainerFamily::SandboxProxy),
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

impl SandboxSession {
    /// Runs one step to completion in its own container. A non-zero exit code is an
    /// [`ExecutionError::CompilerFailed`] naming the step.
    pub async fn run_step(
        &mut self,
        step: &SandboxStep,
    ) -> Result<ExecutionOutput, ExecutionError> {
        let result = self.run_step_unobserved(step).await;
        self.observe_failure(&result);
        result
    }

    async fn run_step_unobserved(
        &mut self,
        step: &SandboxStep,
    ) -> Result<ExecutionOutput, ExecutionError> {
        self.remove_last_step().await?;
        step.validate(&validate_volumes(&self.spec.volumes)?)?;
        if step.network == SandboxNetwork::Egress && self.spec.egress_hosts.is_empty() {
            return Err(ExecutionError::InvalidInvocation(format!(
                "step {} needs egress, but the sandbox lists no hosts",
                step.name
            )));
        }
        // A step that could outlive the expiry label would be reaped mid-run.
        if unix_timestamp()?.saturating_add(step.limits.timeout_seconds) > self.plan.deadline {
            return Err(ExecutionError::Timeout {
                seconds: self.plan.lifetime_seconds,
            });
        }

        let executor = &self.executor;
        let index = self.steps_run;
        self.steps_run += 1;
        let container_id = executor
            .create_sandbox_container(
                self.plan.container_name(&format!("step-{index}")),
                executor.step_config(step, &self.plan),
                ContainerFamily::SandboxStep,
                &mut self.resources,
            )
            .await?;
        self.unremoved_step = Some(container_id.clone());
        let output = executor
            .run_created_container_within(
                &container_id,
                Bytes::new(),
                ContainerFamily::SandboxStep,
                RunLimits {
                    timeout_seconds: step.limits.timeout_seconds,
                    max_output_bytes: step.limits.max_output_bytes,
                },
            )
            .await;
        // Whatever the outcome, the step must not keep running, or keep its mounts or memory,
        // beside what comes next. A timeout leaves it running, so it is removed here, and the
        // next call tries again if this fails.
        if let Err(error) = self.remove_last_step().await {
            tracing::warn!(container_id, error = ?error, "failed to remove sandbox step");
        }
        let output = output?;
        output.ensure_success(&step.name)?;
        Ok(output)
    }

    async fn remove_last_step(&mut self) -> Result<(), ExecutionError> {
        let Some(container_id) = self.unremoved_step.take() else {
            return Ok(());
        };
        if let Err(error) = self.resources.remove_container(&container_id).await {
            self.unremoved_step = Some(container_id);
            return Err(ExecutionError::Infrastructure(
                error.context("remove the previous sandbox step"),
            ));
        }
        Ok(())
    }

    /// Reads a file, or every regular file under a directory, from one of the volumes.
    pub async fn read_output(
        &mut self,
        output: &SandboxOutput,
    ) -> Result<Vec<SandboxFile>, ExecutionError> {
        let result = async {
            self.remove_last_step().await?;
            output.validate(&validate_volumes(&self.spec.volumes)?)?;
            self.executor
                .read_sandbox_output(&self.holder_id, output)
                .await
        }
        .await;
        self.observe_failure(&result);
        result
    }

    /// Removes everything the sandbox created.
    pub async fn finish(mut self) {
        if let Some(observation) = self.observation.take() {
            observation.finish_result(&Ok::<(), ExecutionError>(()));
        }
        self.resources.remove_all_logged(&self.plan).await;
    }

    /// The first failure decides how the sandbox is counted.
    fn observe_failure<T>(&mut self, result: &Result<T, ExecutionError>) {
        if result.is_err() {
            if let Some(observation) = self.observation.take() {
                observation.finish_result(result);
            }
        }
    }
}

/// Names, labels and lifetime shared by everything one job creates on the Docker host.
struct JobPlan {
    job_id: Uuid,
    lifetime_seconds: u64,
    /// When the owner must be done; the expiry adds a grace period for its cleanup.
    deadline: u64,
    expiry: u64,
}

impl JobPlan {
    fn new(lifetime_seconds: u64) -> Result<Self, ExecutionError> {
        let deadline = unix_timestamp()?.saturating_add(lifetime_seconds);
        Ok(Self {
            job_id: Uuid::new_v4(),
            lifetime_seconds,
            deadline,
            expiry: deadline.saturating_add(CLEANUP_GRACE_SECONDS),
        })
    }

    fn volume_name(&self, volume: &str) -> String {
        format!("{OBJECT_PREFIX}{}-{volume}", self.job_id)
    }

    fn container_name(&self, role: &str) -> String {
        format!("{OBJECT_PREFIX}{}-{role}", self.job_id)
    }

    fn network_name(&self) -> String {
        format!("{OBJECT_PREFIX}{}-egress", self.job_id)
    }

    /// `internal` is what keeps steps off every other network: Docker gives it no gateway.
    fn network_request(&self) -> NetworkCreateRequest {
        NetworkCreateRequest {
            name: self.network_name(),
            driver: Some("bridge".to_string()),
            internal: Some(true),
            labels: Some(self.base_labels()),
            ..Default::default()
        }
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
fn job_lifetime_seconds(
    job: &SandboxJob,
    request_timeout_seconds: u64,
    proxy_ready_timeout_seconds: u64,
) -> u64 {
    let count = |items: usize| u64::try_from(items).unwrap_or(u64::MAX);
    // Create per volume; create and start the holder; create, attach, start, inspect and remove
    // per step; one download per output.
    let mut requests = count(job.volumes.len())
        .saturating_add(2)
        .saturating_add(count(job.steps.len()).saturating_mul(5))
        .saturating_add(count(job.outputs.len()));
    let mut waits = 0;
    if job.uses_egress() {
        // Create the network; create, connect and start the proxy. Its readiness probes share
        // one deadline.
        requests = requests.saturating_add(4);
        waits = proxy_ready_timeout_seconds;
    }
    job.steps.iter().fold(
        requests
            .saturating_mul(request_timeout_seconds)
            .saturating_add(waits),
        |total, step| total.saturating_add(step.limits.timeout_seconds),
    )
}

/// One anchored pattern per host, matched against the whole request target. The target of a
/// `CONNECT` is `host:port`, while a plain request's is an absolute URL, which no pattern matches.
/// Host names are validated to hold only letters, digits, dots and hyphens, so the dot is the only
/// character that needs escaping.
fn proxy_filter(hosts: &[String]) -> String {
    hosts
        .iter()
        .map(|host| format!("^{}:443$\n", host.replace('.', "\\.")))
        .collect()
}

/// Lets through only `CONNECT` requests to port 443 of a listed host. `ConnectPort` alone limits
/// `CONNECT` only, so the filter is applied to the request target (`FilterURLs`) rather than to
/// the host: a plain `GET http://host:port/` would otherwise reach any port of a listed host.
fn proxy_configuration() -> String {
    [
        format!("Port {PROXY_PORT}"),
        "Listen 0.0.0.0".to_string(),
        "Timeout 600".to_string(),
        "MaxClients 64".to_string(),
        "DisableViaHeader Yes".to_string(),
        "FilterDefaultDeny Yes".to_string(),
        "FilterExtended On".to_string(),
        "FilterURLs On".to_string(),
        format!("Filter \"{PROXY_FILTER_PATH}\""),
        "ConnectPort 443".to_string(),
        String::new(),
    ]
    .join("\n")
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
    networks: Vec<String>,
}

impl SandboxResources {
    fn new(docker: Docker) -> Self {
        Self {
            docker,
            containers: Vec::new(),
            volumes: Vec::new(),
            networks: Vec::new(),
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

    async fn remove_all_logged(&mut self, plan: &JobPlan) {
        if let Err(error) = self.remove_all().await {
            tracing::warn!(
                job_id = %plan.job_id,
                error = ?error,
                "eager sandbox cleanup failed; the periodic sweep remains armed"
            );
        }
    }

    async fn remove_all(&mut self) -> anyhow::Result<()> {
        let containers = std::mem::take(&mut self.containers);
        let volumes = std::mem::take(&mut self.volumes);
        let networks = std::mem::take(&mut self.networks);
        remove_resources(&self.docker, containers, volumes, networks).await
    }
}

impl Drop for SandboxResources {
    /// Reached with resources left only when the job's future was dropped mid-run.
    fn drop(&mut self) {
        if self.containers.is_empty() && self.volumes.is_empty() && self.networks.is_empty() {
            return;
        }
        let docker = self.docker.clone();
        let containers = std::mem::take(&mut self.containers);
        let volumes = std::mem::take(&mut self.volumes);
        let networks = std::mem::take(&mut self.networks);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Err(error) = remove_resources(&docker, containers, volumes, networks).await {
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
    networks: Vec<String>,
) -> anyhow::Result<()> {
    let mut first_error = None;
    // Steps before the holder and proxy they depend on; volumes and networks once nothing uses
    // them.
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
    for network in networks {
        if let Err(error) = remove_network(docker, &network).await {
            first_error.get_or_insert(error.context(format!("remove network {network}")));
        }
    }
    first_error.map_or(Ok(()), Err)
}

async fn remove_network(docker: &Docker, name: &str) -> anyhow::Result<()> {
    match retry_connect(docker, || docker.remove_network(name)).await {
        Ok(())
        | Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => Ok(()),
        Err(error) => Err(error.into()),
    }
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

/// Removes expired sandbox egress networks whose owner did not.
pub(super) async fn reap_expired_networks(docker: &Docker) -> anyhow::Result<()> {
    let filters = HashMap::from([("label".to_string(), vec![JOB_LABEL.to_string()])]);
    let options = ListNetworksOptionsBuilder::default()
        .filters(&filters)
        .build();
    let now = unix_timestamp()?;
    let networks = retry_connect(docker, || docker.list_networks(Some(options.clone()))).await?;
    let mut first_error = None;
    for network in networks {
        let Some(name) = network.name.filter(|name| name.starts_with(OBJECT_PREFIX)) else {
            continue;
        };
        let expired = network
            .labels
            .as_ref()
            .and_then(|labels| labels.get(JOB_EXPIRY_LABEL))
            .and_then(|expiry| expiry.parse::<u64>().ok())
            .is_some_and(|expiry| expiry <= now);
        if !expired {
            continue;
        }
        match remove_network(docker, &name).await {
            Ok(()) => tracing::warn!(network = name, "removed expired sandbox network"),
            Err(error) => {
                tracing::error!(network = name, error = ?error, "failed to remove expired sandbox network");
                first_error.get_or_insert(error.context(format!("remove network {name}")));
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
            network: SandboxNetwork::None,
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
            egress_hosts: Vec::new(),
        }
    }

    #[test]
    fn step_container_is_locked_down_and_uses_the_step_image() {
        let executor = executor();
        let job = two_volume_job();
        let plan = JobPlan::new(600).unwrap();
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
    fn egress_step_joins_only_the_job_network_and_is_pointed_at_the_proxy() {
        let executor = executor();
        let mut job = two_volume_job();
        job.steps[0].network = SandboxNetwork::Egress;
        job.egress_hosts = vec!["github.com".to_string()];
        let plan = JobPlan::new(600).unwrap();

        let config = executor.step_config(&job.steps[0], &plan);
        assert_eq!(config.network_disabled, Some(false));
        assert_eq!(
            config.env.unwrap()[..5],
            [
                "HOME=/tmp",
                "HTTPS_PROXY=http://egress-proxy:3128",
                "https_proxy=http://egress-proxy:3128",
                "HTTP_PROXY=http://egress-proxy:3128",
                "http_proxy=http://egress-proxy:3128",
            ]
        );
        let host = config.host_config.unwrap();
        assert_eq!(host.network_mode, Some(plan.network_name()));
        assert_eq!(host.readonly_rootfs, Some(true));
        assert_eq!(host.cap_drop, Some(vec!["ALL".to_string()]));

        let network = plan.network_request();
        assert_eq!(network.name, plan.network_name());
        assert!(network.name.starts_with(OBJECT_PREFIX));
        assert_eq!(network.internal, Some(true));
        assert!(network.labels.unwrap().contains_key(JOB_EXPIRY_LABEL));

        // The other step of the same job keeps no network at all.
        let offline = executor.step_config(&job.steps[1], &plan);
        assert_eq!(offline.network_disabled, Some(true));
        assert_eq!(
            offline.host_config.unwrap().network_mode.as_deref(),
            Some("none")
        );
    }

    #[test]
    fn proxy_allows_only_the_listed_hosts_on_port_443() {
        let executor = executor();
        let mut job = two_volume_job();
        job.steps[0].network = SandboxNetwork::Egress;
        job.egress_hosts = vec!["github.com".to_string(), "static.crates.io".to_string()];
        let plan = JobPlan::new(600).unwrap();

        assert_eq!(
            proxy_filter(&job.egress_hosts),
            "^github\\.com:443$\n^static\\.crates\\.io:443$\n"
        );
        let configuration = proxy_configuration();
        for line in [
            "Port 3128",
            "FilterDefaultDeny Yes",
            "FilterExtended On",
            "FilterURLs On",
            "Filter \"/tmp/tinyproxy-filter\"",
            "ConnectPort 443",
        ] {
            assert!(
                configuration.lines().any(|found| found == line),
                "missing {line:?} in {configuration:?}"
            );
        }

        let config = executor.proxy_config(&job.egress_hosts, &plan);
        assert_eq!(config.image, Some(settings().runner_image));
        assert_eq!(config.user.as_deref(), Some("65532:65532"));
        assert_eq!(config.network_disabled, Some(false));
        assert_eq!(
            config.labels.unwrap()[CONTAINER_FAMILY_LABEL],
            "sandbox_proxy"
        );
        let env = config.env.unwrap();
        assert!(env.contains(&format!(
            "TINYPROXY_FILTER={}",
            proxy_filter(&job.egress_hosts)
        )));
        let host = config.host_config.unwrap();
        assert_eq!(host.network_mode.as_deref(), Some("bridge"));
        assert_eq!(host.readonly_rootfs, Some(true));
        assert_eq!(host.cap_drop, Some(vec!["ALL".to_string()]));
        assert_eq!(host.mounts, Some(Vec::new()));
    }

    #[test]
    fn holder_mounts_every_volume_read_only_on_the_runner_image() {
        let executor = executor();
        let job = two_volume_job();
        let plan = JobPlan::new(600).unwrap();
        let config = executor.holder_config(&job.volumes, &plan);

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
        let plan = JobPlan::new(600).unwrap();
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
        assert_eq!(job_lifetime_seconds(&job, 10, 7), 15 * 10 + 2 * 30);

        // Egress adds four requests and one bounded wait for the proxy.
        job.steps[0].network = SandboxNetwork::Egress;
        job.egress_hosts = vec!["github.com".to_string()];
        assert_eq!(job_lifetime_seconds(&job, 10, 7), 19 * 10 + 7 + 2 * 30);
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

    /// Steps need a digest-pinned image with a shell; the runner image serves unless
    /// `SCV_TEST_LOCAL_DOCKER_STEP_IMAGE` names another one.
    /// The Docker host of the sandbox tests, with a client for checking what is left on it: the
    /// remote host named by `SCV_TEST_DOCKER_ADDR` over SSH, with `SCV_TEST_DOCKER_RUNNER_IMAGE`
    /// and optionally `SCV_TEST_DOCKER_KEY_PATH` and `SCV_TEST_DOCKER_PLATFORM`, or else this
    /// machine's daemon with `SCV_TEST_LOCAL_DOCKER_RUNNER_IMAGE`.
    fn test_host() -> (DockerCompilerExecutor, Docker, String) {
        let Ok(addr) = std::env::var("SCV_TEST_DOCKER_ADDR") else {
            let docker = Docker::connect_with_defaults().unwrap();
            let runner_image = std::env::var("SCV_TEST_LOCAL_DOCKER_RUNNER_IMAGE").unwrap();
            return (local_executor(&docker, &runner_image), docker, runner_image);
        };
        let mut settings = settings();
        settings.addr = addr;
        settings.key_path = std::env::var_os("SCV_TEST_DOCKER_KEY_PATH").map(Into::into);
        settings.runner_image = std::env::var("SCV_TEST_DOCKER_RUNNER_IMAGE").unwrap();
        if let Ok(platform) = std::env::var("SCV_TEST_DOCKER_PLATFORM") {
            settings.platform = platform;
        }
        let docker = Docker::connect_with_ssh(
            &settings.addr,
            120,
            bollard::API_DEFAULT_VERSION,
            std::env::var("SCV_TEST_DOCKER_KEY_PATH").ok(),
        )
        .unwrap();
        let runner_image = settings.runner_image.clone();
        (
            DockerCompilerExecutor::new(settings).unwrap(),
            docker,
            runner_image,
        )
    }

    fn step_image(runner_image: &str) -> String {
        std::env::var("SCV_TEST_LOCAL_DOCKER_STEP_IMAGE")
            .unwrap_or_else(|_| runner_image.to_string())
    }

    /// The Docker tests each assert that nothing of theirs is left on the host, so they must not
    /// see one another's sandboxes.
    static LOCAL_DOCKER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
        let networks = docker
            .list_networks(Some(
                ListNetworksOptionsBuilder::default()
                    .filters(&filters)
                    .build(),
            ))
            .await
            .unwrap()
            .into_iter()
            .filter_map(|network| network.name);
        containers.chain(volumes).chain(networks).collect()
    }

    #[tokio::test]
    #[ignore = "requires a test Docker host"]
    async fn docker_sandbox_session_chooses_steps_as_it_goes() {
        let _exclusive = LOCAL_DOCKER.lock().await;
        let (executor, docker, runner_image) = test_host();
        let image = step_image(&runner_image);
        let step = |name: &str, script: &str, mounts| SandboxStep {
            image: image.clone(),
            ..step(name, script, mounts)
        };
        let output = |path: &str| SandboxOutput {
            name: "file".to_string(),
            volume: "data".to_string(),
            path: path.to_string(),
            max_bytes: 64 * 1024,
        };

        let mut session = executor
            .start_sandbox(SandboxSpec {
                volumes: vec![volume("data", 8 << 20)],
                egress_hosts: Vec::new(),
                lifetime_seconds: 120,
            })
            .await
            .unwrap();
        session
            .run_step(&step(
                "write",
                "echo second > /data/choice",
                vec![mount("data", "/data", false)],
            ))
            .await
            .unwrap();

        // The next step is built from what the first one left behind.
        let choice = session.read_output(&output("choice")).await.unwrap();
        let script = format!(
            "echo {} > /data/result",
            String::from_utf8_lossy(&choice[0].content).trim()
        );
        session
            .run_step(&step("act", &script, vec![mount("data", "/data", false)]))
            .await
            .unwrap();
        assert_eq!(
            session.read_output(&output("result")).await.unwrap()[0].content,
            Bytes::from_static(b"second\n")
        );

        // A step that runs out of time is stopped and removed before anything else starts.
        let mut stuck = step(
            "stuck",
            "echo started > /data/stuck; sleep 600",
            vec![mount("data", "/data", false)],
        );
        stuck.limits.timeout_seconds = 3;
        assert!(matches!(
            session.run_step(&stuck).await,
            Err(ExecutionError::Timeout { seconds: 3 })
        ));
        let steps_left = leftover_sandbox_objects(&docker)
            .await
            .into_iter()
            .filter(|name| name.contains("-step-"))
            .collect::<Vec<_>>();
        assert_eq!(steps_left, Vec::<String>::new());
        assert_eq!(
            session.read_output(&output("stuck")).await.unwrap()[0].content,
            Bytes::from_static(b"started\n")
        );

        // A step that could outlive the sandbox is refused, as is egress without listed hosts.
        let mut slow = step("slow", "true", Vec::new());
        slow.limits.timeout_seconds = 3600;
        assert!(matches!(
            session.run_step(&slow).await,
            Err(ExecutionError::Timeout { seconds: 120 })
        ));
        let mut online = step("online", "true", Vec::new());
        online.network = SandboxNetwork::Egress;
        assert!(matches!(
            session.run_step(&online).await,
            Err(ExecutionError::InvalidInvocation(_))
        ));

        session.finish().await;
        assert_eq!(
            leftover_sandbox_objects(&docker).await,
            Vec::<String>::new()
        );
    }

    /// Needs a runner image that ships tinyproxy, and reaches github.com.
    #[tokio::test]
    #[ignore = "requires a test Docker host and network access"]
    async fn docker_sandbox_egress_is_limited_to_listed_hosts() {
        let _exclusive = LOCAL_DOCKER.lock().await;
        let (executor, docker, runner_image) = test_host();
        let image = step_image(&runner_image);
        let script = r#"
            probe() {
                exec 3<>/dev/tcp/egress-proxy/3128 || return 1
                printf 'CONNECT %s:%s HTTP/1.1\r\nHost: %s:%s\r\n\r\n' "$1" "$2" "$1" "$2" >&3
                read -r -t 20 line <&3
                exec 3>&-
                echo "$line" | cut -d' ' -f2
            }
            get() {
                exec 3<>/dev/tcp/egress-proxy/3128 || return 1
                printf 'GET %s HTTP/1.1\r\nHost: github.com\r\n\r\n' "$1" >&3
                read -r -t 20 line <&3
                exec 3>&-
                echo "$line" | cut -d' ' -f2
            }
            echo "listed=$(probe github.com 443)"
            echo "unlisted=$(probe example.com 443)"
            echo "lookalike=$(probe github.com.example.com 443)"
            echo "other-port=$(probe github.com 22)"
            echo "plain=$(get http://github.com/)"
            echo "plain-other-port=$(get http://github.com:22/)"
            if getent hosts example.com >/dev/null; then echo dns=open; else echo dns=closed; fi
            if timeout 5 bash -c 'exec 3<>/dev/tcp/1.1.1.1/443' 2>/dev/null; then
                echo direct=open
            else
                echo direct=closed
            fi
            echo "proxy=$HTTPS_PROXY"
        "#;
        let job = SandboxJob {
            volumes: Vec::new(),
            steps: vec![
                SandboxStep {
                    image: image.clone(),
                    command: vec![
                        "/bin/bash".to_string(),
                        "-c".to_string(),
                        script.to_string(),
                    ],
                    network: SandboxNetwork::Egress,
                    ..step("online", "", Vec::new())
                },
                SandboxStep {
                    image: image.clone(),
                    ..step(
                        "offline",
                        "if [ -e /sys/class/net/eth0 ]; then echo online; else echo offline; fi",
                        Vec::new(),
                    )
                },
            ],
            outputs: Vec::new(),
            egress_hosts: vec!["github.com".to_string()],
        };

        let output = executor.run_sandbox_job(job).await.unwrap();
        assert_eq!(
            String::from_utf8_lossy(&output.steps[0].stdout),
            "listed=200\nunlisted=403\nlookalike=403\nother-port=403\nplain=403\n\
             plain-other-port=403\ndns=closed\ndirect=closed\n\
             proxy=http://egress-proxy:3128\n"
        );
        assert_eq!(output.steps[1].stdout, Bytes::from_static(b"offline\n"));
        assert_eq!(
            leftover_sandbox_objects(&docker).await,
            Vec::<String>::new()
        );
    }

    #[tokio::test]
    #[ignore = "requires a test Docker host"]
    async fn docker_sandbox_job_round_trip() {
        let _exclusive = LOCAL_DOCKER.lock().await;
        let (executor, docker, runner_image) = test_host();
        let image = step_image(&runner_image);
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
            egress_hosts: Vec::new(),
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
