// SPDX-License-Identifier: LicenseRef-Blockscout

//! The sandbox steps of one verification.
//!
//! Everything from the request reaches a step as a positional argument of a fixed script, never
//! as part of the script text. The clone and fetch steps, which have network, run nothing from
//! the repository; the build steps do, offline, and they see the sources read-only.
//!
//! The build output depends on the path it is built at, so the build steps reproduce the layout
//! of cargo-stylus's own reproducible build: the directory it mounts is at `/source`, with
//! `target` inside it.

use compiler_runner::{
    SandboxLimits, SandboxMount, SandboxNetwork, SandboxOutput, SandboxSpec, SandboxStep,
    SandboxVolume,
};
use semver::Version;
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
/// written and as resolved, which rules out symlinked components, and its manifest has to be a
/// regular file.
///
/// The build runs in the project or one of its parents, so every directory from the repository
/// root to the project gets a real `target` directory to mount the build directory at: the
/// sources are read-only afterwards. Each of them that holds a `rust-toolchain.toml` is printed
/// as `toolchain-file:<directory>`, relative to the repository.
const CLONE_SCRIPT: &str = r#"
set -uf
git -c protocol.file.allow=never clone --quiet --no-tags \
    --separate-git-dir=/source/git -- "$1" /source/repo || exit 66
git -C /source/repo -c advice.detachedHead=false checkout --quiet --detach "$2" -- || exit 65
project="/source/repo${3:+/$3}"
[ -d "$project" ] || exit 67
[ "$(cd "$project" && pwd -P)" = "$project" ] || exit 67
[ -f "$project/Cargo.toml" ] && [ ! -L "$project/Cargo.toml" ] || exit 68
toolchain_files=0
visit() {
    if [ -e "$1/target" ] || [ -L "$1/target" ]; then
        [ -d "$1/target" ] && [ ! -L "$1/target" ] || exit 69
    else
        mkdir "$1/target" || exit 69
    fi
    if [ -f "$1/rust-toolchain.toml" ] && [ ! -L "$1/rust-toolchain.toml" ]; then
        echo "toolchain-file:${1#/source/repo/}"
        toolchain_files=$((toolchain_files + 1))
    fi
}
directory=/source/repo
visit "$directory/"
IFS=/
for name in $3; do
    directory="$directory/$name"
    visit "$directory"
done
[ "$toolchain_files" -gt 0 ] || exit 64
"#;
/// Run from `/`, outside the repository: cargo and rustup look for their configuration from the
/// working directory up, so the repository's `.cargo/config.toml` and `rust-toolchain.toml`,
/// which can name programs to run, are not read. A dependency source the repository configures
/// there is therefore not fetched, and such a project fails to build offline.
const FETCH_SCRIPT: &str =
    r#"exec cargo fetch --locked --manifest-path "/source/repo${1:+/$1}/Cargo.toml""#;
/// Prints the manifest of the project's workspace root; reading the manifests is enough to find
/// it. Run from `/` for the same reason as the fetch.
const LOCATE_SCRIPT: &str = r#"exec cargo locate-project --workspace --message-format plain \
    --manifest-path "/source/repo${1:+/$1}/Cargo.toml""#;
/// `$1` is the contract to build, or empty when the directory holds only the project.
const BUILD_SCRIPT: &str = r#"
if [ -n "$1" ]; then set -- --contract "$1"; else set --; fi
exec cargo stylus get-initcode "$@" --output /out/initcode
"#;
const EXPORT_ABI_SCRIPT: &str = r#"
if [ -n "$1" ]; then set -- --contract "$1"; else set --; fi
exec cargo stylus export-abi "$@"
"#;

/// The image a verification builds with, and the cargo-stylus version and toolchain it uses there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Tools {
    pub image: String,
    pub cargo_stylus_version: Version,
    pub toolchain: Version,
}

/// The first cargo-stylus version that builds from the workspace root rather than the project.
const WORKSPACE_BUILDS_FROM: Version = Version::new(0, 10, 0);

/// What cargo-stylus's reproducible build mounts at `/source`, and the contract it then names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BuildRoot {
    /// Relative to the repository; empty for the repository itself.
    pub directory: String,
    pub contract: Option<String>,
}

/// The directories, relative to the repository, that the clone step found a toolchain file in,
/// from the repository root down to the project.
pub(crate) fn toolchain_directories(clone_stdout: &str) -> Vec<String> {
    clone_stdout
        .lines()
        .filter_map(|line| line.strip_prefix("toolchain-file:"))
        .map(|directory| directory.trim_end_matches('/').to_string())
        .collect()
}

/// Before 0.10 cargo-stylus mounts the directory it is run in, which is the project's. From 0.10
/// on it mounts the workspace root of the project and passes `--contract`.
pub(crate) fn builds_from_workspace_root(cargo_stylus_version: &Version) -> bool {
    cargo_stylus_version >= &WORKSPACE_BUILDS_FROM
}

impl BuildRoot {
    pub(crate) fn project(path_prefix: &str) -> Self {
        Self {
            directory: path_prefix.to_string(),
            contract: None,
        }
    }

    pub(crate) fn workspace(workspace_root: String, package_name: String) -> Self {
        Self {
            directory: workspace_root,
            contract: Some(package_name),
        }
    }

    /// The toolchain file the reproducible build ends up with. cargo-stylus installs the
    /// toolchain of the nearest file above the project, and rustup, run at `/source`, prefers a
    /// file there.
    pub(crate) fn toolchain_directory<'a>(&self, found: &'a [String]) -> Option<&'a str> {
        found
            .iter()
            .find(|directory| **directory == self.directory)
            .or_else(|| found.last())
            .map(String::as_str)
    }

    fn source_subpath(&self) -> String {
        match self.directory.as_str() {
            "" => "repo".to_string(),
            directory => format!("repo/{directory}"),
        }
    }
}

/// The workspace root the locate step printed, relative to the repository. It has to be the
/// project or one of its parents inside the repository.
pub(crate) fn workspace_root(locate_stdout: &str, path_prefix: &str) -> Option<String> {
    let manifest = locate_stdout.lines().next()?.trim_end();
    let directory = manifest.strip_suffix("/Cargo.toml")?;
    let relative = match directory.strip_prefix(REPOSITORY_ROOT)? {
        "" => "",
        relative => relative.strip_prefix('/')?,
    };
    let is_normal = relative.is_empty()
        || relative
            .split('/')
            .all(|name| !matches!(name, "" | "." | ".."));
    let contains_project = relative.is_empty()
        || path_prefix == relative
        || path_prefix
            .strip_prefix(relative)
            .is_some_and(|rest| rest.starts_with('/'));
    (is_normal && contains_project).then(|| relative.to_string())
}

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
/// Finding the workspace root only reads manifests.
const LOCATE_TIMEOUT_SECONDS: u64 = 60;

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
                .saturating_add(LOCATE_TIMEOUT_SECONDS)
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
    pub(crate) fn fetch_step(&self, tools: &Tools, path_prefix: &str) -> SandboxStep {
        SandboxStep {
            name: "fetch".to_string(),
            image: tools.image.clone(),
            command: script(FETCH_SCRIPT, &[path_prefix]),
            env: vec![("RUSTUP_TOOLCHAIN".to_string(), tools.toolchain.to_string())],
            working_dir: "/".to_string(),
            network: SandboxNetwork::Egress,
            mounts: vec![
                mount(SOURCE_VOLUME, SOURCE_ROOT, true),
                mount(REGISTRY_VOLUME, REGISTRY_ROOT, false),
            ],
            limits: self.limits(self.fetch_memory_bytes, self.fetch_timeout_seconds),
        }
    }

    /// Runs nothing from the repository, without network.
    pub(crate) fn locate_step(
        &self,
        image: &str,
        toolchain: &Version,
        path_prefix: &str,
    ) -> SandboxStep {
        SandboxStep {
            name: "locate".to_string(),
            image: image.to_string(),
            command: script(LOCATE_SCRIPT, &[path_prefix]),
            env: vec![("RUSTUP_TOOLCHAIN".to_string(), toolchain.to_string())],
            working_dir: "/".to_string(),
            network: SandboxNetwork::None,
            mounts: vec![mount(SOURCE_VOLUME, SOURCE_ROOT, true)],
            limits: self.limits(self.clone_memory_bytes, LOCATE_TIMEOUT_SECONDS),
        }
    }

    pub(crate) fn build_step(&self, tools: &Tools, root: &BuildRoot) -> SandboxStep {
        self.offline_step("build", BUILD_SCRIPT, tools, root)
    }

    pub(crate) fn export_abi_step(&self, tools: &Tools, root: &BuildRoot) -> SandboxStep {
        self.offline_step("export-abi", EXPORT_ABI_SCRIPT, tools, root)
    }

    /// No network, and nothing writable but the build directory and the output volume. An image
    /// that holds several cargo-stylus versions runs the one `CARGO_STYLUS_VERSION` names. The
    /// toolchain is named outright: it is the one the reproducible build would use, and rustup
    /// then does not try to install what the repository's toolchain file lists.
    fn offline_step(
        &self,
        name: &str,
        script_text: &str,
        tools: &Tools,
        root: &BuildRoot,
    ) -> SandboxStep {
        SandboxStep {
            name: name.to_string(),
            image: tools.image.clone(),
            command: script(script_text, &[root.contract.as_deref().unwrap_or("")]),
            env: vec![
                ("CARGO_NET_OFFLINE".to_string(), "true".to_string()),
                (
                    "CARGO_STYLUS_VERSION".to_string(),
                    tools.cargo_stylus_version.to_string(),
                ),
                ("RUSTUP_TOOLCHAIN".to_string(), tools.toolchain.to_string()),
            ],
            working_dir: SOURCE_ROOT.to_string(),
            network: SandboxNetwork::None,
            mounts: vec![
                SandboxMount {
                    subpath: Some(root.source_subpath()),
                    ..mount(SOURCE_VOLUME, SOURCE_ROOT, true)
                },
                mount(REGISTRY_VOLUME, REGISTRY_ROOT, true),
                mount(BUILD_VOLUME, &format!("{SOURCE_ROOT}/target"), false),
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
        subpath: None,
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

    fn project(directory: &str) -> BuildRoot {
        BuildRoot::project(directory)
    }

    fn tools() -> Tools {
        Tools {
            cargo_stylus_version: "0.6.2".parse().unwrap(),
            toolchain: "1.80.0".parse().unwrap(),
            image: IMAGE.to_string(),
        }
    }

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
        let step = JobSettings::default().fetch_step(&tools(), "a/b");
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
    fn build_steps_name_the_cargo_stylus_version() {
        let settings = JobSettings::default();
        for step in [
            settings.build_step(&tools(), &project("p")),
            settings.export_abi_step(&tools(), &project("p")),
        ] {
            assert!(
                step.env
                    .contains(&("CARGO_STYLUS_VERSION".to_string(), "0.6.2".to_string())),
                "{}: {:?}",
                step.name,
                step.env
            );
        }
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
            settings.fetch_step(&tools(), "p"),
            settings.build_step(&tools(), &project("p")),
            settings.export_abi_step(&tools(), &project("p")),
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
            settings.fetch_step(&tools(), "p").network,
            SandboxNetwork::Egress
        );
        for step in [
            settings.build_step(&tools(), &project("p")),
            settings.export_abi_step(&tools(), &project("p")),
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
    fn builds_run_at_source_like_the_reproducible_build() {
        let settings = JobSettings::default();
        let step = |root: &BuildRoot| settings.build_step(&tools(), root);
        let mount_of = |step: &SandboxStep, volume: &str| {
            step.mounts
                .iter()
                .find(|mount| mount.volume == volume)
                .cloned()
                .unwrap()
        };

        for (directory, subpath) in [
            ("", "repo"),
            ("examples/single_call", "repo/examples/single_call"),
        ] {
            let step = step(&project(directory));
            assert_eq!(step.working_dir, "/source");
            let source = mount_of(&step, SOURCE_VOLUME);
            assert_eq!(source.path, "/source");
            assert_eq!(source.subpath.as_deref(), Some(subpath));
            assert!(source.read_only);
            assert_eq!(mount_of(&step, BUILD_VOLUME).path, "/source/target");
            assert_eq!(step.command[4..], [""]);
        }

        let workspace = BuildRoot {
            directory: "contracts".to_string(),
            contract: Some("counter".to_string()),
        };
        for step in [
            settings.build_step(&tools(), &workspace),
            settings.export_abi_step(&tools(), &workspace),
        ] {
            assert_eq!(step.command[4..], ["counter"]);
        }
        compiler_runner::SandboxJob {
            volumes: settings.sandbox().volumes,
            steps: vec![step(&workspace)],
            outputs: Vec::new(),
            egress_hosts: Vec::new(),
        }
        .validate()
        .unwrap();
    }

    #[test]
    fn cargo_stylus_builds_from_the_workspace_root_from_0_10() {
        for (version, from_workspace) in [
            ("0.5.12", false),
            ("0.6.3", false),
            ("0.10.0", true),
            ("0.10.10", true),
        ] {
            assert_eq!(
                builds_from_workspace_root(&version.parse().unwrap()),
                from_workspace,
                "{version}"
            );
        }
    }

    #[test]
    fn the_toolchain_file_is_the_one_the_reproducible_build_ends_up_with() {
        let found = |directories: &[&str]| -> Vec<String> {
            directories.iter().map(|d| d.to_string()).collect()
        };
        let workspace = BuildRoot::workspace("ws".to_string(), "counter".to_string());
        // A file at the build root wins; otherwise the one nearest the project.
        assert_eq!(
            workspace.toolchain_directory(&found(&["ws", "ws/counter"])),
            Some("ws")
        );
        assert_eq!(
            workspace.toolchain_directory(&found(&["", "ws/counter"])),
            Some("ws/counter")
        );
        assert_eq!(
            project("ws/counter").toolchain_directory(&found(&[""])),
            Some("")
        );
        assert_eq!(project("p").toolchain_directory(&found(&[])), None);

        assert_eq!(
            toolchain_directories("noise\ntoolchain-file:\ntoolchain-file:ws/counter\n"),
            ["", "ws/counter"]
        );
    }

    #[test]
    fn workspace_root_must_hold_the_project_inside_the_repository() {
        let root = |stdout: &str, prefix: &str| workspace_root(stdout, prefix);
        assert_eq!(root("/source/repo/Cargo.toml\n", ""), Some(String::new()));
        assert_eq!(
            root("/source/repo/Cargo.toml\n", "a/b"),
            Some(String::new())
        );
        assert_eq!(
            root("/source/repo/a/Cargo.toml\n", "a/b"),
            Some("a".to_string())
        );
        assert_eq!(
            root("/source/repo/a/b/Cargo.toml\nignored\n", "a/b"),
            Some("a/b".to_string())
        );
        for (stdout, prefix) in [
            ("", "a"),
            ("/source/Cargo.toml\n", ""),
            ("/source/repository/Cargo.toml\n", ""),
            ("/source/repo/ab/Cargo.toml\n", "a/b"),
            ("/source/repo/a/../Cargo.toml\n", "a"),
            ("/source/repo/c/Cargo.toml\n", "a/b"),
            ("/source/repo/a/b/c/Cargo.toml\n", "a/b"),
            ("/source/repo/a/Cargo.lock\n", "a"),
            ("/etc/Cargo.toml\n", ""),
        ] {
            assert_eq!(root(stdout, prefix), None, "{stdout:?} for {prefix:?}");
        }
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
        assert_eq!(sandbox.lifetime_seconds, 300 + 60 + 600 + 2 * 900 + 300);
        assert!(settings.fetch_memory_bytes > settings.registry_volume_bytes);
        assert!(settings.build_memory_bytes > settings.build_volume_bytes);
    }
}
