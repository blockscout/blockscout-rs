use crate::{
    proto::{health_actix::route_health, health_server::HealthServer},
    services::HealthService,
    settings::Settings,
};
use blockscout_service_launcher::{launcher, launcher::LaunchSettings, tracing};
use std::collections::BTreeMap;

use crate::{
    config::ChainsSettings,
    services::{ProxyService, SolidityVerifierService, VyperVerifierService},
};
use proxy_verifier_proto::blockscout::proxy_verifier::v1::{
    proxy_actix::route_proxy, proxy_server::ProxyServer,
    solidity_verifier_actix::route_solidity_verifier,
    solidity_verifier_server::SolidityVerifierServer, vyper_verifier_actix::route_vyper_verifier,
    vyper_verifier_server::VyperVerifierServer,
};
use std::sync::Arc;

const SERVICE_NAME: &str = "proxy_verifier";

#[derive(Clone)]
struct Router {
    health: Arc<HealthService>,
    proxy: Arc<ProxyService>,
    solidity_verifier: Arc<SolidityVerifierService>,
    vyper_verifier: Arc<VyperVerifierService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(ProxyServer::from_arc(self.proxy.clone()))
            .add_service(SolidityVerifierServer::from_arc(
                self.solidity_verifier.clone(),
            ))
            .add_service(VyperVerifierServer::from_arc(self.vyper_verifier.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config
            .configure(|config| route_health(config, self.health.clone()))
            .configure(|config| route_proxy(config, self.proxy.clone()))
            .configure(|config| route_solidity_verifier(config, self.solidity_verifier.clone()))
            .configure(|config| route_vyper_verifier(config, self.vyper_verifier.clone()));
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let chains = ChainsSettings::new(settings.chains_config)?;

    let eth_bytecode_db_client = {
        let config = eth_bytecode_db_proto::http_client::Config::new(
            settings.eth_bytecode_db.http_url.into(),
        )
        .with_retry_middleware(settings.eth_bytecode_db.max_retries)
        .probe_url(settings.eth_bytecode_db.probe_url)
        .set_api_key(settings.eth_bytecode_db.api_key);

        Arc::new(eth_bytecode_db_proto::http_client::Client::new(config).await)
    };

    let health = Arc::new(HealthService::default());
    let proxy = Arc::new(ProxyService::new(
        chains.clone(),
        eth_bytecode_db_client.clone(),
    ));

    let blockscout_clients = {
        let mut clients = BTreeMap::new();
        for (id, settings) in chains.into_inner() {
            let client = proxy_verifier_logic::blockscout::Client::new(
                settings.api_url,
                settings
                    .sensitive_api_key
                    .expect("sensitive_api_key value must not be null"),
            )
            .await;

            clients.insert(id, client);
        }
        Arc::new(clients)
    };

    let solidity_verifier = Arc::new(SolidityVerifierService::new(
        blockscout_clients.clone(),
        eth_bytecode_db_client.clone(),
    ));
    let vyper_verifier = Arc::new(VyperVerifierService::new(
        blockscout_clients,
        eth_bytecode_db_client,
    ));

    let router = Router {
        health,
        proxy,
        solidity_verifier,
        vyper_verifier,
    };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
        graceful_shutdown: Default::default(),
    };

    launcher::launch(launch_settings, http_router, grpc_router).await
}
