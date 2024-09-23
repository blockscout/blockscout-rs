// Adapted from https://github.com/OffchainLabs/cargo-stylus

use anyhow::Context;
use semver::Version;
use std::{io::Write, path::Path};
use tokio::{io::AsyncWriteExt, process::Command};

pub async fn run_reproducible(
    cargo_stylus_version: &Version,
    toolchain: &Version,
    dir: &Path,
    command_line: &[&str],
) -> Result<String, anyhow::Error> {
    tracing::trace!(
        "Running reproducible Stylus command with cargo-stylus {}, toolchain {}",
        cargo_stylus_version,
        toolchain
    );
    let mut command = vec!["cargo", "stylus"];
    for s in command_line.iter() {
        command.push(s);
    }
    create_image(cargo_stylus_version, toolchain).await?;
    run_in_docker_container(cargo_stylus_version, toolchain, dir, &command).await
}

fn version_to_image_name(cargo_stylus_version: &Version, toolchain: &Version) -> String {
    format!(
        "blockscout/cargo-stylus:{}-rust-{}",
        cargo_stylus_version, toolchain
    )
}

fn validate_docker_output(output: &std::process::Output) -> anyhow::Result<String> {
    if !output.status.success() {
        let stderr =
            std::str::from_utf8(&output.stderr).context("failed to read Docker command stderr")?;
        if stderr.contains("Cannot connect to the Docker daemon") {
            tracing::error!("Docker is not found in the system");
            anyhow::bail!("Docker not running");
        }
        tracing::error!("Docker command failed: {stderr}");
        anyhow::bail!("Docker command failed: {stderr}");
    }

    let stdout = std::str::from_utf8(&output.stdout).context("failed to read Docker stdout")?;
    Ok(stdout.to_string())
}

async fn image_exists(name: &str) -> Result<bool, anyhow::Error> {
    let output = Command::new("docker")
        .arg("images")
        .arg(name)
        .output()
        .await
        .context("failed to execute Docker command")?;

    let stdout = validate_docker_output(&output)?;

    Ok(stdout.chars().filter(|c| *c == '\n').count() > 1)
}

async fn create_image(
    cargo_stylus_version: &Version,
    toolchain: &Version,
) -> Result<(), anyhow::Error> {
    let name = version_to_image_name(cargo_stylus_version, toolchain);
    if image_exists(&name).await? {
        return Ok(());
    }

    tracing::trace!(
        "Building Docker image for cargo-stylus {}, Rust toolchain {}",
        cargo_stylus_version,
        toolchain
    );
    let mut child = Command::new("docker")
        .arg("build")
        .arg("-t")
        .arg(name)
        .arg(".")
        .arg("-f-")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("failed to execute Docker command")?;

    let mut dockerfile = Vec::<u8>::new();
    write!(
            dockerfile,
            "\
            FROM --platform=linux/amd64 offchainlabs/cargo-stylus-base:{cargo_stylus_version} as base
            RUN rustup toolchain install {toolchain}-x86_64-unknown-linux-gnu
            RUN rustup default {toolchain}-x86_64-unknown-linux-gnu
            RUN rustup target add wasm32-unknown-unknown
            RUN rustup component add rust-src --toolchain {toolchain}-x86_64-unknown-linux-gnu
            ",
        ).expect("write into the vector should not fail");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&dockerfile)
        .await
        .context("failed to write dockerfile content into docker process stdin")?;
    child.wait().await.context("wait failed")?;

    Ok(())
}

async fn run_in_docker_container(
    cargo_stylus_version: &Version,
    toolchain: &Version,
    dir: &Path,
    command_line: &[&str],
) -> Result<String, anyhow::Error> {
    let name = version_to_image_name(cargo_stylus_version, toolchain);
    if !image_exists(&name).await? {
        anyhow::bail!("Docker image {name} doesn't exist");
    }

    let output = Command::new("docker")
        .arg("run")
        .arg("--network")
        .arg("host")
        .arg("-w")
        .arg("/source")
        .arg("-v")
        .arg(format!("{}:/source", dir.as_os_str().to_str().unwrap()))
        .arg("--rm")
        .arg(name)
        .args(command_line)
        .output()
        .await
        .context("failed to execute Docker command")?;

    validate_docker_output(&output)
}
