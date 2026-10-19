// SPDX-License-Identifier: LicenseRef-Blockscout

//! The sandbox steps of one verification.
//!
//! Everything from the request reaches a step as a positional argument of a fixed script, never
//! as part of the script text. The clone and fetch steps, which have network, run nothing from
//! the repository; the build steps do, offline, and they see the sources read-only.

use compiler_runner::{
    SandboxLimits, SandboxMount, SandboxNetwork, SandboxOutput, SandboxSpec, SandboxStep,
    SandboxVolume,
};
use serde::Deserialize;

pub(crate) const SOURCE_VOLUME: &str = "source";
const REGISTRY_VOLUME: &str = "registry";
const BUILD_VOLUME: &str = "build";
const OUT_VOLUME: &str = "out";

const SOURCE_ROOT: &str = "/source";
/// The working tree. The git directory is kept beside it, so the tree holds only project files.
const REPOSITORY_ROOT: &str = "/source/repo";
const REGISTRY_ROOT: &str = "/usr/local/cargo/registry";
const OUT_ROOT: &str = "/out";
const INITCODE_FILE: &str = "initcode";

/// Exit codes of the clone step, which runs no repository code and can therefore be believed.
pub(crate) mod clone_exit {
    pub const TOOLCHAIN_NOT_FOUND: i64 = 64;
    pub const COMMIT_NOT_FOUND: i64 = 65;
    pub const REPOSITORY_NOT_FOUND: i64 = 66;
    pub const INVALID_PATH_PREFIX: i64 = 67;
    pub const PACKAGE_NOT_FOUND: i64 = 68;
    pub const INVALID_TARGET: i64 = 69;
}

/// `$1` repository URL, `$2` commit, `$3` path prefix. The project path has to be the same as
/// written and as resolved, which rules out symlinked components, and the two manifests have to
/// be regular files. `target` is created here because the sources are read-only afterwards.
const CLONE_SCRIPT: &str = r#"
set -u
git -c protocol.file.allow=never clone --quiet --no-tags \
    --separate-git-dir=/source/git -- "$1" /source/repo || exit 66
git -C /source/repo -c advice.detachedHead=false checkout --quiet --detach "$2" -- || exit 65
project="/source/repo${3:+/$3}"
[ -d "$project" ] || exit 67
[ "$(cd "$project" && pwd -P)" = "$project" ] || exit 67
[ -f "$project/rust-toolchain.toml" ] && [ ! -L "$project/rust-toolchain.toml" ] || exit 64
[ -f "$project/Cargo.toml" ] && [ ! -L "$project/Cargo.toml" ] || exit 68
if [ -e "$project/target" ] || [ -L "$project/target" ]; then
    [ -d "$project/target" ] && [ ! -L "$project/target" ] || exit 69
else
    mkdir "$project/target" || exit 69
fi
"#;
/// Run from `/`, outside the repository: cargo and rustup look for their configuration from the
/// working directory up, so the repository's `.cargo/config.toml` and `rust-toolchain.toml`,
/// which can name programs to run, are not read. A dependency source the repository configures
/// there is therefore not fetched, and such a project fails to build offline.
const FETCH_SCRIPT: &str =
    r#"exec cargo fetch --locked --manifest-path "/source/repo${1:+/$1}/Cargo.toml""#;
const BUILD_SCRIPT: &str =
    r#"cd "/source/repo${1:+/$1}" && exec cargo stylus get-initcode --output /out/initcode"#;
const EXPORT_ABI_SCRIPT: &str = r#"cd "/source/repo${1:+/$1}" && exec cargo stylus export-abi"#;

/// Sizes and limits of one verification. A step's memory limit also covers what it writes to the
/// volumes, so `fetch_memory_bytes` has to exceed `registry_volume_bytes` and
/// `build_memory_bytes` has to exceed `build_volume_bytes`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct JobSettings {
    pub source_volume_bytes: u64,
    pub registry_volume_bytes: u64,
    pub build_volume_bytes: u64,
    pub clone_memory_bytes: u64,
    pub fetch_memory_bytes: u64,
    pub build_memory_bytes: u64,
    pub clone_timeout_seconds: u64,
    pub fetch_timeout_seconds: u64,
    pub build_timeout_seconds: u64,
    pub nano_cpus: u64,
    pub pids_limit: u64,
    /// Bounds the project files read back for the response.
    pub max_source_bytes: usize,
    /// Hosts the clone and fetch steps may reach, on port 443.
    pub egress_hosts: Vec<String>,
}

impl Default for JobSettings {
    fn default() -> Self {
        const MIB: u64 = 1024 * 1024;
        Self {
            source_volume_bytes: 256 * MIB,
            registry_volume_bytes: 1024 * MIB,
            build_volume_bytes: 3 * 1024 * MIB,
            clone_memory_bytes: 512 * MIB,
            fetch_memory_bytes: 2 * 1024 * MIB,
            build_memory_bytes: 7 * 1024 * MIB,
            clone_timeout_seconds: 300,
            fetch_timeout_seconds: 600,
            build_timeout_seconds: 900,
            nano_cpus: 4_000_000_000,
            pids_limit: 1024,
            max_source_bytes: 16 * 1024 * 1024,
            egress_hosts: [
                "github.com",
                "index.crates.io",
                "static.crates.io",
                "crates.io",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

const OUT_VOLUME_BYTES: u64 = 16 * 1024 * 1024;
const MAX_STEP_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_INITCODE_BYTES: usize = 1024 * 1024;
/// Docker requests around the steps: creating the volumes, the holder and the proxy, and reading
/// the outputs.
const OVERHEAD_SECONDS: u64 = 300;

impl JobSettings {
    pub(crate) fn sandbox(&self) -> SandboxSpec {
        let volume = |name: &str, size_bytes| SandboxVolume {
            name: name.to_string(),
            size_bytes,
        };
        SandboxSpec {
            volumes: vec![
                volume(SOURCE_VOLUME, self.source_volume_bytes),
                volume(REGISTRY_VOLUME, self.registry_volume_bytes),
                volume(BUILD_VOLUME, self.build_volume_bytes),
                volume(OUT_VOLUME, OUT_VOLUME_BYTES),
            ],
            egress_hosts: self.egress_hosts.clone(),
            // The build limit is counted twice: once for the build, once for the ABI export.
            lifetime_seconds: self
                .clone_timeout_seconds
                .saturating_add(self.fetch_timeout_seconds)
                .saturating_add(self.build_timeout_seconds.saturating_mul(2))
                .saturating_add(OVERHEAD_SECONDS),
        }
    }

    fn limits(&self, memory_bytes: u64, timeout_seconds: u64) -> SandboxLimits {
        SandboxLimits {
            memory_bytes,
            nano_cpus: self.nano_cpus,
            pids_limit: self.pids_limit,
            timeout_seconds,
            max_output_bytes: MAX_STEP_OUTPUT_BYTES,
        }
    }

    /// Runs nothing from the repository: it is the only step that may write to the sources.
    pub(crate) fn clone_step(
        &self,
        image: &str,
        repository_url: &str,
        commit: &str,
        path_prefix: &str,
    ) -> SandboxStep {
        SandboxStep {
            name: "clone".to_string(),
            image: image.to_string(),
            command: script(CLONE_SCRIPT, &[repository_url, commit, path_prefix]),
            env: vec![("GIT_TERMINAL_PROMPT".to_string(), "0".to_string())],
            working_dir: "/".to_string(),
            network: SandboxNetwork::Egress,
            mounts: vec![mount(SOURCE_VOLUME, SOURCE_ROOT, false)],
            limits: self.limits(self.clone_memory_bytes, self.clone_timeout_seconds),
        }
    }

    /// Has network, so it must run nothing from the repository: see [`FETCH_SCRIPT`]. The
    /// toolchain is the image's, named outright rather than taken from the repository.
    pub(crate) fn fetch_step(
        &self,
        image: &str,
        toolchain: &str,
        path_prefix: &str,
    ) -> SandboxStep {
        SandboxStep {
            name: "fetch".to_string(),
            image: image.to_string(),
            command: script(FETCH_SCRIPT, &[path_prefix]),
            env: vec![("RUSTUP_TOOLCHAIN".to_string(), toolchain.to_string())],
            working_dir: "/".to_string(),
            network: SandboxNetwork::Egress,
            mounts: vec![
                mount(SOURCE_VOLUME, SOURCE_ROOT, true),
                mount(REGISTRY_VOLUME, REGISTRY_ROOT, false),
            ],
            limits: self.limits(self.fetch_memory_bytes, self.fetch_timeout_seconds),
        }
    }

    pub(crate) fn build_step(&self, image: &str, path_prefix: &str) -> SandboxStep {
        self.offline_step("build", BUILD_SCRIPT, image, path_prefix)
    }

    pub(crate) fn export_abi_step(&self, image: &str, path_prefix: &str) -> SandboxStep {
        self.offline_step("export-abi", EXPORT_ABI_SCRIPT, image, path_prefix)
    }

    /// No network, and nothing writable but the build directory and the output volume.
    fn offline_step(
        &self,
        name: &str,
        script_text: &str,
        image: &str,
        path_prefix: &str,
    ) -> SandboxStep {
        SandboxStep {
            name: name.to_string(),
            image: image.to_string(),
            command: script(script_text, &[path_prefix]),
            env: vec![("CARGO_NET_OFFLINE".to_string(), "true".to_string())],
            working_dir: "/".to_string(),
            network: SandboxNetwork::None,
            mounts: vec![
                mount(SOURCE_VOLUME, SOURCE_ROOT, true),
                mount(REGISTRY_VOLUME, REGISTRY_ROOT, true),
                mount(
                    BUILD_VOLUME,
                    &format!("{}/target", project_root(path_prefix)),
                    false,
                ),
                mount(OUT_VOLUME, OUT_ROOT, false),
            ],
            limits: self.limits(self.build_memory_bytes, self.build_timeout_seconds),
        }
    }

    pub(crate) fn manifest_output(&self, path_prefix: &str, file_name: &str) -> SandboxOutput {
        SandboxOutput {
            name: "manifest".to_string(),
            volume: SOURCE_VOLUME.to_string(),
            path: format!("{}/{file_name}", project_in_volume(path_prefix)),
            max_bytes: MAX_MANIFEST_BYTES,
        }
    }

    pub(crate) fn sources_output(&self, path_prefix: &str) -> SandboxOutput {
        SandboxOutput {
            name: "sources".to_string(),
            volume: SOURCE_VOLUME.to_string(),
            path: project_in_volume(path_prefix),
            max_bytes: self.max_source_bytes,
        }
    }

    pub(crate) fn initcode_output(&self) -> SandboxOutput {
        SandboxOutput {
            name: "initcode".to_string(),
            volume: OUT_VOLUME.to_string(),
            path: INITCODE_FILE.to_string(),
            max_bytes: MAX_INITCODE_BYTES,
        }
    }
}

fn script(text: &str, arguments: &[&str]) -> Vec<String> {
    ["/bin/sh", "-c", text, "sh"]
        .into_iter()
        .chain(arguments.iter().copied())
        .map(String::from)
        .collect()
}

fn mount(volume: &str, path: &str, read_only: bool) -> SandboxMount {
    SandboxMount {
        volume: volume.to_string(),
        path: path.to_string(),
        read_only,
    }
}

fn project_root(path_prefix: &str) -> String {
    match path_prefix {
        "" => REPOSITORY_ROOT.to_string(),
        prefix => format!("{REPOSITORY_ROOT}/{prefix}"),
    }
}

/// The project directory relative to the source volume's root.
fn project_in_volume(path_prefix: &str) -> String {
    project_root(path_prefix)
        .trim_start_matches(SOURCE_ROOT)
        .trim_start_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const IMAGE: &str =
        "tools@sha256:0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn request_values_are_script_arguments_not_script_text() {
        let hostile = "x\"; rm -rf / #";
        let step = JobSettings::default().clone_step(IMAGE, hostile, "$(id)", "a b");
        assert_eq!(step.command[..4], ["/bin/sh", "-c", CLONE_SCRIPT, "sh"]);
        assert_eq!(step.command[4..], [hostile, "$(id)", "a b"]);
        assert!(!CLONE_SCRIPT.contains(hostile));
    }

    #[test]
    fn fetch_step_reads_no_configuration_from_the_repository() {
        let step = JobSettings::default().fetch_step(IMAGE, "1.80.0", "a/b");
        assert_eq!(step.working_dir, "/");
        assert!(!FETCH_SCRIPT.contains("cd "));
        assert!(FETCH_SCRIPT.contains("--manifest-path"));
        assert_eq!(step.command[4..], ["a/b"]);
        assert_eq!(
            step.env,
            [("RUSTUP_TOOLCHAIN".to_string(), "1.80.0".to_string())]
        );
    }

    #[test]
    fn only_the_clone_step_can_write_to_the_sources() {
        let settings = JobSettings::default();
        let writable_source = |step: &SandboxStep| {
            step.mounts
                .iter()
                .any(|mount| mount.volume == SOURCE_VOLUME && !mount.read_only)
        };
        assert!(writable_source(&settings.clone_step(IMAGE, "u", "c", "p")));
        for step in [
            settings.fetch_step(IMAGE, "1.80.0", "p"),
            settings.build_step(IMAGE, "p"),
            settings.export_abi_step(IMAGE, "p"),
        ] {
            assert!(!writable_source(&step), "{} can write sources", step.name);
        }
    }

    #[test]
    fn steps_that_run_project_code_after_fetching_have_no_network() {
        let settings = JobSettings::default();
        assert_eq!(
            settings.clone_step(IMAGE, "u", "c", "p").network,
            SandboxNetwork::Egress
        );
        assert_eq!(
            settings.fetch_step(IMAGE, "1.80.0", "p").network,
            SandboxNetwork::Egress
        );
        for step in [
            settings.build_step(IMAGE, "p"),
            settings.export_abi_step(IMAGE, "p"),
        ] {
            assert_eq!(step.network, SandboxNetwork::None, "{}", step.name);
            let registry = step
                .mounts
                .iter()
                .find(|mount| mount.volume == REGISTRY_VOLUME)
                .unwrap();
            assert!(registry.read_only);
        }
    }

    #[test]
    fn build_directory_is_mounted_inside_the_project() {
        let settings = JobSettings::default();
        let target = |prefix| {
            settings
                .build_step(IMAGE, prefix)
                .mounts
                .into_iter()
                .find(|mount| mount.volume == BUILD_VOLUME)
                .unwrap()
                .path
        };
        assert_eq!(target(""), "/source/repo/target");
        assert_eq!(
            target("examples/single_call"),
            "/source/repo/examples/single_call/target"
        );
    }

    #[test]
    fn outputs_are_addressed_relative_to_their_volume() {
        let settings = JobSettings::default();
        assert_eq!(settings.sources_output("").path, "repo");
        assert_eq!(settings.sources_output("a/b").path, "repo/a/b");
        assert_eq!(
            settings.manifest_output("a/b", "Cargo.toml").path,
            "repo/a/b/Cargo.toml"
        );
        assert_eq!(settings.initcode_output().path, "initcode");
    }

    #[test]
    fn default_sandbox_is_valid_and_its_memory_limits_cover_the_volumes() {
        let settings = JobSettings::default();
        let sandbox = settings.sandbox();
        sandbox.validate().unwrap();
        assert_eq!(sandbox.lifetime_seconds, 300 + 600 + 2 * 900 + 300);
        assert!(settings.fetch_memory_bytes > settings.registry_volume_bytes);
        assert!(settings.build_memory_bytes > settings.build_volume_bytes);
    }
}
