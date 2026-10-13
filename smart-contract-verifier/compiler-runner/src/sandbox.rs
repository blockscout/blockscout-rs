// SPDX-License-Identifier: LicenseRef-Blockscout

//! Multi-step jobs for programs that are not a single compiler invocation.
//!
//! A job is a list of steps that run one after another, each in its own locked-down container, and
//! share data only through the job's volumes. The steps run code the caller does not trust, so
//! every step is a non-root, read-only-root, capability-free container, and a step sees a volume
//! only if it mounts it.

use crate::ExecutionError;
use bytes::Bytes;
use std::{
    collections::HashSet,
    path::{Component, Path},
};

const MAX_NAME_LEN: usize = 32;

/// Scratch space shared by the steps of one job. It lives in memory, is owned by the job user and
/// cannot grow past `size_bytes`; whatever a step writes to it counts against that step's
/// [`SandboxLimits::memory_bytes`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxVolume {
    pub name: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxMount {
    /// Name of one of the job's volumes.
    pub volume: String,
    /// Absolute path inside the step's container. A path inside another mount must already exist
    /// there when that outer mount is read-only.
    pub path: String,
    pub read_only: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SandboxNetwork {
    #[default]
    None,
    /// Reaches the job's [`SandboxJob::egress_hosts`] on port 443 through a filtering proxy, and
    /// nothing else: the step's only network has no route out.
    Egress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SandboxLimits {
    /// Covers the step's processes and everything it writes to volumes and `/tmp`.
    pub memory_bytes: u64,
    pub nano_cpus: u64,
    pub pids_limit: u64,
    pub timeout_seconds: u64,
    /// Combined stdout and stderr.
    pub max_output_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxStep {
    /// Names the step in errors; not sent to the container.
    pub name: String,
    /// Must be pinned by sha256 digest and already present on the Docker host.
    pub image: String,
    pub command: Vec<String>,
    pub env: Vec<(String, String)>,
    pub working_dir: String,
    pub network: SandboxNetwork,
    pub mounts: Vec<SandboxMount>,
    pub limits: SandboxLimits,
}

/// A file, or every regular file under a directory, to read back once all steps have succeeded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxOutput {
    pub name: String,
    pub volume: String,
    /// Relative to the volume root.
    pub path: String,
    /// Bounds the archive the Docker host streams back, so it includes tar framing.
    pub max_bytes: usize,
}

/// What a sandbox holds for its whole life, when its steps are decided as it goes rather than
/// listed up front as in a [`SandboxJob`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxSpec {
    pub volumes: Vec<SandboxVolume>,
    /// Lowercase host names that steps with [`SandboxNetwork::Egress`] may connect to. Left
    /// empty, the sandbox has no proxy and such steps are refused.
    pub egress_hosts: Vec<String>,
    /// Everything the sandbox created is reclaimed this long after it started, even if its owner
    /// died. A step whose own timeout does not fit in the time left is refused.
    pub lifetime_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxJob {
    pub volumes: Vec<SandboxVolume>,
    pub steps: Vec<SandboxStep>,
    pub outputs: Vec<SandboxOutput>,
    /// Lowercase host names that steps with [`SandboxNetwork::Egress`] may connect to.
    pub egress_hosts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxFile {
    /// Path as the archive reports it: the requested file's name, or `<directory>/<path>`.
    pub path: String,
    pub content: Bytes,
}

#[derive(Clone, Debug)]
pub struct SandboxJobOutput {
    /// One entry per step, in order.
    pub steps: Vec<crate::ExecutionOutput>,
    /// One entry per requested output, in order. Only regular files are returned: the steps are
    /// untrusted, and a link could name anything.
    pub outputs: Vec<Vec<SandboxFile>>,
}

impl SandboxJob {
    pub fn validate(&self) -> Result<(), ExecutionError> {
        if self.steps.is_empty() {
            return Err(invalid("a sandbox job needs at least one step"));
        }

        let volumes = validate_volumes(&self.volumes)?;

        for step in &self.steps {
            step.validate(&volumes)?;
        }

        if self.uses_egress() == self.egress_hosts.is_empty() {
            return Err(invalid(
                "egress hosts must be listed exactly when a step uses the egress network",
            ));
        }
        validate_hosts(&self.egress_hosts)?;

        let mut outputs = HashSet::new();
        for output in &self.outputs {
            validate_name("output", &output.name)?;
            if !outputs.insert(output.name.as_str()) {
                return Err(invalid(format!("duplicate output: {}", output.name)));
            }
            output.validate(&volumes)?;
        }
        Ok(())
    }
}

impl SandboxSpec {
    pub fn validate(&self) -> Result<(), ExecutionError> {
        validate_volumes(&self.volumes)?;
        validate_hosts(&self.egress_hosts)?;
        if self.lifetime_seconds == 0 {
            return Err(invalid("a sandbox needs a lifetime"));
        }
        Ok(())
    }
}

impl SandboxOutput {
    pub(crate) fn validate(&self, volumes: &HashSet<&str>) -> Result<(), ExecutionError> {
        validate_name("output", &self.name)?;
        if !volumes.contains(self.volume.as_str()) {
            return Err(invalid(format!(
                "output {} reads an undeclared volume: {}",
                self.name, self.volume
            )));
        }
        if !is_normal_relative_path(&self.path) {
            return Err(invalid(format!(
                "output {} needs a normalized path relative to its volume",
                self.name
            )));
        }
        if self.max_bytes == 0 {
            return Err(invalid(format!("output {} has no size limit", self.name)));
        }
        Ok(())
    }
}

pub(crate) fn validate_volumes(volumes: &[SandboxVolume]) -> Result<HashSet<&str>, ExecutionError> {
    let mut names = HashSet::new();
    for volume in volumes {
        validate_name("volume", &volume.name)?;
        if !names.insert(volume.name.as_str()) {
            return Err(invalid(format!("duplicate volume: {}", volume.name)));
        }
        if volume.size_bytes == 0 {
            return Err(invalid(format!("volume {} has no size", volume.name)));
        }
    }
    Ok(names)
}

fn validate_hosts(hosts: &[String]) -> Result<(), ExecutionError> {
    match hosts.iter().find(|host| !is_host_name(host)) {
        Some(host) => Err(invalid(format!(
            "egress host must be a lowercase DNS name: {host:?}"
        ))),
        None => Ok(()),
    }
}

impl SandboxJob {
    pub(crate) fn uses_egress(&self) -> bool {
        self.steps
            .iter()
            .any(|step| step.network == SandboxNetwork::Egress)
    }
}

impl SandboxStep {
    pub(crate) fn validate(&self, volumes: &HashSet<&str>) -> Result<(), ExecutionError> {
        validate_name("step", &self.name)?;
        let step = &self.name;
        if !is_pinned_image(&self.image) {
            return Err(invalid(format!(
                "step {step} image must be pinned by sha256 digest"
            )));
        }
        if self
            .command
            .first()
            .is_none_or(|program| program.is_empty())
        {
            return Err(invalid(format!("step {step} has no command")));
        }
        if !is_normal_absolute_path(&self.working_dir) {
            return Err(invalid(format!(
                "step {step} needs a normalized absolute working directory"
            )));
        }
        if self
            .env
            .iter()
            .any(|(name, _)| name.is_empty() || name.contains('='))
        {
            return Err(invalid(format!(
                "step {step} has an invalid environment variable name"
            )));
        }

        let mut paths = HashSet::new();
        for mount in &self.mounts {
            if !volumes.contains(mount.volume.as_str()) {
                return Err(invalid(format!(
                    "step {step} mounts an undeclared volume: {}",
                    mount.volume
                )));
            }
            if !is_normal_absolute_path(&mount.path) || mount.path == "/" {
                return Err(invalid(format!(
                    "step {step} needs a normalized absolute mount path below the root"
                )));
            }
            // `/tmp` is the step's private home directory.
            if Path::new(&mount.path).starts_with("/tmp") {
                return Err(invalid(format!("step {step} must not mount over /tmp")));
            }
            if !paths.insert(mount.path.as_str()) {
                return Err(invalid(format!("step {step} mounts {} twice", mount.path)));
            }
        }

        let limits = &self.limits;
        if limits.memory_bytes == 0
            || limits.nano_cpus == 0
            || limits.pids_limit == 0
            || limits.timeout_seconds == 0
            || limits.max_output_bytes == 0
            || i64::try_from(limits.memory_bytes).is_err()
            || i64::try_from(limits.nano_cpus).is_err()
            || i64::try_from(limits.pids_limit).is_err()
        {
            return Err(invalid(format!(
                "step {step} limits must be positive and fit the Docker API"
            )));
        }
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> ExecutionError {
    ExecutionError::InvalidInvocation(message.into())
}

/// Names end up in Docker object names and labels.
fn validate_name(kind: &str, name: &str) -> Result<(), ExecutionError> {
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "{kind} name must be 1-{MAX_NAME_LEN} characters of a-z, 0-9 and '-': {name:?}"
        )))
    }
}

pub(crate) fn is_pinned_image(image: &str) -> bool {
    image.rsplit_once("@sha256:").is_some_and(|(name, digest)| {
        !name.is_empty()
            && digest.len() == 64
            && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

/// The proxy filter is built from these names, so only what a DNS name can hold is accepted: no
/// wildcards, ports, addresses or pattern characters.
fn is_host_name(host: &str) -> bool {
    host.len() <= 253
        && host.contains('.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
        && host.bytes().any(|byte| byte.is_ascii_lowercase())
}

fn is_normal_absolute_path(path: &str) -> bool {
    if path == "/" {
        return true;
    }
    let mut components = Path::new(path).components();
    components.next() == Some(Component::RootDir)
        && components.all(|component| matches!(component, Component::Normal(_)))
        && !path.ends_with('/')
}

fn is_normal_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.ends_with('/')
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn pinned(image: &str) -> String {
        format!("{image}@sha256:{}", "0".repeat(64))
    }

    pub(crate) fn limits() -> SandboxLimits {
        SandboxLimits {
            memory_bytes: 256 * 1024 * 1024,
            nano_cpus: 1_000_000_000,
            pids_limit: 64,
            timeout_seconds: 30,
            max_output_bytes: 1024 * 1024,
        }
    }

    fn job() -> SandboxJob {
        SandboxJob {
            volumes: vec![
                SandboxVolume {
                    name: "source".to_string(),
                    size_bytes: 16 * 1024 * 1024,
                },
                SandboxVolume {
                    name: "build".to_string(),
                    size_bytes: 64 * 1024 * 1024,
                },
            ],
            steps: vec![
                SandboxStep {
                    name: "fetch".to_string(),
                    image: pinned("tools"),
                    command: vec!["fetch".to_string()],
                    env: vec![("HOME".to_string(), "/tmp".to_string())],
                    working_dir: "/source".to_string(),
                    network: SandboxNetwork::Egress,
                    mounts: vec![SandboxMount {
                        volume: "source".to_string(),
                        path: "/source".to_string(),
                        read_only: false,
                    }],
                    limits: limits(),
                },
                SandboxStep {
                    name: "build".to_string(),
                    image: pinned("tools"),
                    command: vec!["build".to_string()],
                    env: vec![],
                    working_dir: "/source/project".to_string(),
                    network: SandboxNetwork::None,
                    mounts: vec![
                        SandboxMount {
                            volume: "source".to_string(),
                            path: "/source".to_string(),
                            read_only: true,
                        },
                        SandboxMount {
                            volume: "build".to_string(),
                            path: "/source/project/target".to_string(),
                            read_only: false,
                        },
                    ],
                    limits: limits(),
                },
            ],
            outputs: vec![SandboxOutput {
                name: "artifact".to_string(),
                volume: "build".to_string(),
                path: "out/artifact".to_string(),
                max_bytes: 1024 * 1024,
            }],
            egress_hosts: vec!["github.com".to_string(), "static.crates.io".to_string()],
        }
    }

    fn rejected(job: &SandboxJob) -> String {
        match job.validate() {
            Err(ExecutionError::InvalidInvocation(message)) => message,
            other => panic!("expected an invalid invocation, got {other:?}"),
        }
    }

    #[test]
    fn two_step_job_with_a_nested_mount_is_valid() {
        job().validate().unwrap();
    }

    #[test]
    fn a_job_needs_steps_and_declared_volumes() {
        let mut no_steps = job();
        no_steps.steps.clear();
        assert!(rejected(&no_steps).contains("at least one step"));

        let mut undeclared = job();
        undeclared.steps[0].mounts[0].volume = "other".to_string();
        assert!(rejected(&undeclared).contains("undeclared volume"));

        let mut duplicate = job();
        duplicate.volumes[1].name = "source".to_string();
        assert!(rejected(&duplicate).contains("duplicate volume"));

        let mut unsized_volume = job();
        unsized_volume.volumes[0].size_bytes = 0;
        assert!(rejected(&unsized_volume).contains("no size"));
    }

    #[test]
    fn images_must_be_pinned_by_digest() {
        for image in ["tools", "tools:latest", "tools@sha256:abc", "@sha256:"] {
            let mut job = job();
            job.steps[0].image = image.to_string();
            assert!(rejected(&job).contains("pinned by sha256"), "{image}");
        }
    }

    #[test]
    fn mount_paths_are_absolute_normalized_and_off_tmp() {
        for path in [
            "source",
            "/",
            "/source/../etc",
            "/source/",
            "/tmp",
            "/tmp/x",
        ] {
            let mut job = job();
            job.steps[0].mounts[0].path = path.to_string();
            assert!(job.validate().is_err(), "mount path {path:?} was accepted");
        }

        let mut twice = job();
        let mount = twice.steps[0].mounts[0].clone();
        twice.steps[0].mounts.push(mount);
        assert!(rejected(&twice).contains("twice"));
    }

    #[test]
    fn outputs_stay_inside_their_volume() {
        for path in ["", "/etc/passwd", "../x", "a/../b", "out/"] {
            let mut job = job();
            job.outputs[0].path = path.to_string();
            assert!(job.validate().is_err(), "output path {path:?} was accepted");
        }

        let mut undeclared = job();
        undeclared.outputs[0].volume = "other".to_string();
        assert!(rejected(&undeclared).contains("undeclared volume"));
    }

    #[test]
    fn egress_hosts_are_plain_dns_names_listed_only_when_used() {
        for host in [
            "GitHub.com",
            "github.com:443",
            "*.github.com",
            ".github.com",
            "github..com",
            "localhost",
            "10.0.0.1",
            "git hub.com",
            "github.com$",
            "-a.com",
        ] {
            let mut job = job();
            job.egress_hosts = vec![host.to_string()];
            assert!(job.validate().is_err(), "egress host {host:?} was accepted");
        }

        let mut unused = job();
        unused.steps[0].network = SandboxNetwork::None;
        assert!(rejected(&unused).contains("exactly when"));

        let mut missing = job();
        missing.egress_hosts.clear();
        assert!(rejected(&missing).contains("exactly when"));
    }

    #[test]
    fn names_commands_and_limits_are_checked() {
        let mut name = job();
        name.steps[0].name = "Fetch Step".to_string();
        assert!(rejected(&name).contains("step name"));

        let mut command = job();
        command.steps[0].command.clear();
        assert!(rejected(&command).contains("no command"));

        let mut env = job();
        env.steps[0].env.push(("A=B".to_string(), String::new()));
        assert!(rejected(&env).contains("environment variable"));

        let mut limit = job();
        limit.steps[0].limits.timeout_seconds = 0;
        assert!(rejected(&limit).contains("limits"));

        let mut oversized = job();
        oversized.steps[0].limits.memory_bytes = u64::MAX;
        assert!(rejected(&oversized).contains("limits"));
    }
}
