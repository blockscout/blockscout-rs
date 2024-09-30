use anyhow::Context;
use bollard::{
    container::{self, AttachContainerOptions, CreateContainerOptions, LogOutput},
    image::{BuildImageOptions, BuilderVersion},
    models::HostConfig,
    Docker,
};
use futures_util::stream::StreamExt;
use semver::Version;
use std::{path::Path, str};
use uuid::Uuid;

pub async fn run_reproducible(
    cargo_stylus_version: &Version,
    toolchain: &Version,
    dir: &Path,
    command_line: &[&str],
) -> Result<String, anyhow::Error> {
    let docker =
        Docker::connect_with_http_defaults().context("failed to connect to docker daemon")?;
    tracing::trace!(
        "Running reproducible Stylus command with cargo-stylus {}, toolchain {}",
        cargo_stylus_version,
        toolchain
    );
    let mut command = vec!["cargo", "stylus"];
    for s in command_line.iter() {
        command.push(s);
    }
    let image_name = version_to_image_name(cargo_stylus_version, toolchain);
    create_image(&docker, &image_name, cargo_stylus_version, toolchain)
        .await
        .context("creating image")?;

    let container_id = create_container(&docker, &image_name, dir, &command)
        .await
        .context("creating container")?;
    let output = start_container(&docker, &container_id)
        .await
        .context("running container")?;

    Ok(output)
}

fn version_to_image_name(cargo_stylus_version: &Version, toolchain: &Version) -> String {
    format!(
        "blockscout/cargo-stylus:{}-rust-{}",
        cargo_stylus_version, toolchain
    )
}

async fn image_exists(docker: &Docker, name: &str) -> Result<bool, anyhow::Error> {
    match docker.inspect_image(name).await {
        Ok(_) => Ok(true),
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => Ok(false),
        Err(err) => Err(err).context("failed to inspect docker image"),
    }
}

async fn create_image(
    docker: &Docker,
    image_name: &str,
    cargo_stylus_version: &Version,
    toolchain: &Version,
) -> Result<(), anyhow::Error> {
    if image_exists(docker, image_name)
        .await
        .context("check if image exists")?
    {
        return Ok(());
    }

    tracing::trace!(
        "Building Docker image for cargo-stylus {}, Rust toolchain {}",
        cargo_stylus_version,
        toolchain
    );

    let dockerfile = format!(
        "FROM offchainlabs/cargo-stylus-base:{cargo_stylus_version} as base
RUN rustup toolchain install {toolchain}-x86_64-unknown-linux-gnu
RUN rustup default {toolchain}-x86_64-unknown-linux-gnu
RUN rustup target add wasm32-unknown-unknown
RUN rustup component add rust-src --toolchain {toolchain}-x86_64-unknown-linux-gnu
"
    );

    let content = build_tar_with_dockerfile(&dockerfile)?;
    let build_image_options = BuildImageOptions {
        t: image_name.to_string(),
        dockerfile: "Dockerfile".to_string(),
        version: BuilderVersion::BuilderV1,
        networkmode: "host".to_string(),
        pull: true,
        rm: true,
        forcerm: true,
        platform: "linux/amd64".to_string(),
        ..Default::default()
    };

    let mut image_build_stream =
        docker.build_image(build_image_options, None, Some(content.into()));

    let mut output = vec![];
    while let Some(result) = image_build_stream.next().await {
        match result {
            Ok(info) => {
                if let Some(value) = info.stream {
                    output.push(value)
                }
            }
            Err(bollard::errors::Error::DockerStreamError { error }) => {
                output.push(error);
                let output = output.join("");
                tracing::error!(
                    image_name = image_name,
                    output = output,
                    "error while building an image"
                );
                anyhow::bail!(
                    "error while building an image; image_name={image_name}, output={output}"
                );
            }
            Err(err) => {
                let output = output.join("");
                tracing::error!(
                    image_name = image_name,
                    output = output,
                    error = err.to_string(),
                    "unknown error while building an image"
                );
                anyhow::bail!("unknown error while building an image; image_name={image_name}, output={output}, error={err}");
            }
        }
    }

    Ok(())
}

fn build_tar_with_dockerfile(content: &str) -> Result<Vec<u8>, anyhow::Error> {
    let mut header = tar::Header::new_gnu();
    header
        .set_path("Dockerfile")
        .context("set dockerfile path in the header")?;
    header.set_size(content.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    let mut tar = tar::Builder::new(Vec::new());
    tar.append(&header, content.as_bytes())
        .context("append dockerfile")?;

    tar.into_inner()
        .context("convert tar into inner representation")
}

async fn create_container(
    docker: &Docker,
    image_name: &str,
    dir: &Path,
    command_line: &[&str],
) -> Result<String, anyhow::Error> {
    let container_suffix = Uuid::new_v4();
    let container_name = format!(
        "{}-{container_suffix}",
        image_name.replace(|c: char| !c.is_alphanumeric(), "_")
    );

    let options = CreateContainerOptions {
        name: container_name.clone(),
        ..Default::default()
    };
    let config = container::Config {
        image: Some(image_name),
        working_dir: Some("/source"),
        host_config: Some(HostConfig {
            binds: Some(vec![format!(
                "{}:/source",
                dir.as_os_str().to_str().unwrap()
            )]),
            network_mode: Some("host".to_string()),
            auto_remove: Some(true),
            ..Default::default()
        }),
        cmd: Some(command_line.into()),
        ..Default::default()
    };

    let container = docker.create_container(Some(options), config).await?;

    Ok(container.id)
}

async fn start_container(docker: &Docker, container_id: &str) -> Result<String, anyhow::Error> {
    docker.start_container::<String>(container_id, None).await?;
    let mut attach_results = docker
        .attach_container::<String>(
            container_id,
            Some(AttachContainerOptions {
                stdout: Some(true),
                stderr: Some(true),
                stream: Some(true),
                logs: Some(true),
                ..Default::default()
            }),
        )
        .await?;

    let mut container_output = vec![];
    while let Some(result) = attach_results.output.next().await {
        match result {
            Ok(output) => match output {
                LogOutput::StdErr { message } => container_output.push(message),
                LogOutput::StdOut { message } => container_output.push(message),
                _ => (),
            },
            Err(err) => {
                tracing::error!(
                    err = err.to_string(),
                    "reading output from attached container failed"
                );
                Err(err).context("reading output from attached container")?
            }
        }
    }

    Ok(container_output.into_iter().filter_map(|output| {match str::from_utf8(&output) {
        Ok(s) => Some(s.to_string()),
        Err(err) => {
            tracing::warn!(line=?output, err=err.to_string(), "converting output line to utf8 string failed");
            None
        }
    }}).collect::<Vec<_>>().join(""))
}
