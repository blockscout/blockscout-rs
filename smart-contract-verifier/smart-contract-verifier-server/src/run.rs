// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{
    proto::{
        health_server::HealthServer,
        solidity_verifier_actix::route_solidity_verifier,
        solidity_verifier_server::SolidityVerifierServer,
        sourcify_verifier_actix::route_sourcify_verifier,
        sourcify_verifier_server::SourcifyVerifierServer,
        vyper_verifier_actix::route_vyper_verifier,
        vyper_verifier_server::VyperVerifierServer,
        zksync::solidity::{
            zk_sync_solidity_verifier_actix::route_zk_sync_solidity_verifier,
            zk_sync_solidity_verifier_server::ZkSyncSolidityVerifierServer,
        },
    },
    services::{
        route_health, zksync_solidity_verifier, HealthService, SolidityVerifierService,
        SourcifyVerifierService, VyperVerifierService,
    },
    settings::{CompilerExecutionSettings, Settings},
};
use blockscout_service_launcher::launcher::{self, LaunchSettings};
use smart_contract_verifier::{
    CompilerExecutor, ConcurrencyLimitedCompilerExecutor, DockerCompilerExecutor,
    NativeCompilerExecutor,
};
use std::{sync::Arc, time::Duration};

#[derive(Clone)]
struct HttpRouter {
    solidity_verifier: Option<Arc<SolidityVerifierService>>,
    vyper_verifier: Option<Arc<VyperVerifierService>>,
    sourcify_verifier: Option<Arc<SourcifyVerifierService>>,
    zksync_solidity_verifier: Option<Arc<zksync_solidity_verifier::Service>>,
    health: Arc<HealthService>,
}

impl launcher::HttpRouter for HttpRouter {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        let service_config =
            service_config.configure(|config| route_health(config, self.health.clone()));

        let service_config = if let Some(solidity) = &self.solidity_verifier {
            service_config.configure(|config| route_solidity_verifier(config, solidity.clone()))
        } else {
            service_config
        };
        let service_config = if let Some(vyper) = &self.vyper_verifier {
            service_config.configure(|config| route_vyper_verifier(config, vyper.clone()))
        } else {
            service_config
        };
        let service_config = if let Some(sourcify) = &self.sourcify_verifier {
            service_config.configure(|config| route_sourcify_verifier(config, sourcify.clone()))
        } else {
            service_config
        };
        let service_config = if let Some(zksync_solidity) = &self.zksync_solidity_verifier {
            service_config.configure(|config| {
                route_zk_sync_solidity_verifier(config, zksync_solidity.clone())
            })
        } else {
            service_config
        };

        let _ = service_config;
    }
}

fn grpc_router(
    solidity_verifier: Option<Arc<SolidityVerifierService>>,
    vyper_verifier: Option<Arc<VyperVerifierService>>,
    sourcify_verifier: Option<Arc<SourcifyVerifierService>>,
    zksync_solidity_verifier: Option<Arc<zksync_solidity_verifier::Service>>,
    health: Arc<HealthService>,
) -> tonic::transport::server::Router {
    tonic::transport::Server::builder()
        .add_service(HealthServer::from_arc(health))
        .add_optional_service(solidity_verifier.map(SolidityVerifierServer::from_arc))
        .add_optional_service(vyper_verifier.map(VyperVerifierServer::from_arc))
        .add_optional_service(sourcify_verifier.map(SourcifyVerifierServer::from_arc))
        .add_optional_service(zksync_solidity_verifier.map(ZkSyncSolidityVerifierServer::from_arc))
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    let compiler_endpoints_enabled =
        settings.solidity.enabled || settings.vyper.enabled || settings.zksync_solidity.enabled;
    let raw_compiler_executor: Arc<dyn CompilerExecutor> = match &settings.compilers.execution {
        CompilerExecutionSettings::Disabled if compiler_endpoints_enabled => anyhow::bail!(
            "compiler execution is disabled while a compiler-backed endpoint is enabled"
        ),
        CompilerExecutionSettings::Disabled => Arc::new(NativeCompilerExecutor::default()),
        CompilerExecutionSettings::Native {
            execution_timeout_seconds,
            max_output_bytes,
        } => Arc::new(NativeCompilerExecutor::new(
            Duration::from_secs(*execution_timeout_seconds),
            *max_output_bytes,
        )?),
        CompilerExecutionSettings::Docker { .. } => Arc::new(DockerCompilerExecutor::new(
            settings
                .compilers
                .execution
                .docker_executor_settings()
                .expect("docker settings must exist for Docker mode"),
        )?),
    };
    let compiler_executor: Arc<dyn CompilerExecutor> =
        Arc::new(ConcurrencyLimitedCompilerExecutor::new(
            raw_compiler_executor,
            settings.compilers.max_threads,
        ));

    let solidity_verifier = match settings.solidity.enabled {
        true => Some(Arc::new(
            SolidityVerifierService::new_with_admitted_executor(
                settings.solidity,
                compiler_executor.clone(),
            )
            .await?,
        )),
        false => None,
    };
    let vyper_verifier = match settings.vyper.enabled {
        true => Some(Arc::new(
            VyperVerifierService::new_with_admitted_executor(
                settings.vyper,
                compiler_executor.clone(),
            )
            .await?,
        )),
        false => None,
    };
    let sourcify_verifier = match settings.sourcify.enabled {
        true => Some(Arc::new(
            SourcifyVerifierService::new(settings.sourcify).await?,
        )),
        false => None,
    };
    let zksync_solidity_verifier = match settings.zksync_solidity.enabled {
        true => Some(Arc::new(
            zksync_solidity_verifier::Service::new_with_admitted_executor(
                settings.zksync_solidity,
                compiler_executor.clone(),
            )
            .await?,
        )),
        false => None,
    };
    let health = Arc::new(HealthService::new(
        compiler_endpoints_enabled.then_some(compiler_executor),
    ));
    let grpc_router = grpc_router(
        solidity_verifier.clone(),
        vyper_verifier.clone(),
        sourcify_verifier.clone(),
        zksync_solidity_verifier.clone(),
        health.clone(),
    );
    let http_router = HttpRouter {
        solidity_verifier,
        vyper_verifier,
        sourcify_verifier,
        zksync_solidity_verifier,
        health,
    };
    let launch_settings = LaunchSettings {
        service_name: "smart_contract_verifier".to_owned(),
        server: settings.server,
        metrics: settings.metrics,
        graceful_shutdown: Default::default(),
    };

    blockscout_service_launcher::tracing::init_logs(
        &launch_settings.service_name,
        &settings.tracing,
        &settings.jaeger,
    )?;

    launcher::launch(launch_settings, http_router, grpc_router).await
}
