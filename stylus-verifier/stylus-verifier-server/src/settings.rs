// SPDX-License-Identifier: LicenseRef-Blockscout

use blockscout_service_launcher::{
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use serde::Deserialize;
use stylus_verifier_logic::{stylus_sdk_rs::VerifierSettings, DockerCompilerExecutorSettings};

#[derive(Debug, Default, Deserialize, Clone, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub server: ServerSettings,
    pub metrics: MetricsSettings,
    pub tracing: TracingSettings,
    pub jaeger: JaegerSettings,
    pub sandbox: SandboxSettings,
}

/// Where and how submitted projects are built. They are never built on this machine.
#[derive(Debug, Default, Deserialize, Clone, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SandboxSettings {
    /// The dedicated Docker host, reached over SSH. Required.
    pub docker: Option<DockerCompilerExecutorSettings>,
    pub verifier: VerifierSettings,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "STYLUS_VERIFIER";

    fn validate(&self) -> anyhow::Result<()> {
        if self.sandbox.docker.is_none() {
            anyhow::bail!(
                "sandbox.docker settings are required: projects are built on a remote Docker host"
            );
        }
        Ok(())
    }
}
