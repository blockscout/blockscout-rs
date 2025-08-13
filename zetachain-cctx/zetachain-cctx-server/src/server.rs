use crate::{
    proto::{
        cctx_info_actix::route_cctx_info,
        cctx_info_server::CctxInfoServer,
        health_actix::route_health,
        health_server::HealthServer,
        stats_actix::route_stats,
        token_info_actix::route_token_info,
        token_info_server::TokenInfoServer,
    },
    services::{cctx::CctxService, stats::StatsService, token::TokenInfoService, HealthService},
    settings::Settings,
};

use blockscout_service_launcher::{
    launcher::{self, GracefulShutdownHandler, LaunchSettings},
    tracing as launcher_tracing,
};

use actix_phoenix_channel::{configure_channel_websocket_route, ChannelCentral};
use zetachain_cctx_logic::channel::Channel;

use sea_orm::DatabaseConnection;
use zetachain_cctx_logic::{
    client::Client,
    database::ZetachainCctxDatabase,
    indexer::Indexer,
};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::
    stats_server::StatsServer
;

use std::sync::Arc;
use tokio::time::Duration;

const SERVICE_NAME: &str = "zetachain_cctx";

#[derive(Clone)]
struct Router {
    health: Arc<HealthService>,
    cctx: Arc<CctxService>,
    stats: Arc<StatsService>,
    token_info: Arc<TokenInfoService>,
    channel: Arc<ChannelCentral<Channel>>,
}



impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(CctxInfoServer::from_arc(self.cctx.clone()))
            .add_service(StatsServer::from_arc(self.stats.clone()))
            .add_service(TokenInfoServer::from_arc(self.token_info.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config.configure(|config| route_cctx_info(config, self.cctx.clone()));
        service_config.configure(|config| route_stats(config, self.stats.clone()));
        service_config.configure(|config| route_token_info(config, self.token_info.clone()));
        service_config.configure(|config| configure_channel_websocket_route(config, self.channel.clone()));

        
    }
}

pub async fn run(
    settings: Settings,
    db: Arc<DatabaseConnection>,
    client: Arc<Client>,
) -> Result<(), anyhow::Error> {
    launcher_tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let database = Arc::new(ZetachainCctxDatabase::new(db.clone(), settings.indexer.zetachain_id));
    let health = Arc::new(HealthService::default());
    let cctx = Arc::new(CctxService::new(database.clone()));
    let stats = Arc::new(StatsService::new(database.clone()));
    let token_info = Arc::new(TokenInfoService::new(database.clone()));

    let channel: Arc<ChannelCentral<Channel>> = Arc::new(ChannelCentral::new(Channel));

    if settings.indexer.enabled {
        let indexer = Indexer::new(
            settings.indexer.clone(),
            client,
            database,
            Arc::new(channel.channel_broadcaster()),
        );
        let restart_interval = settings.restart_interval;
        let restart_on_error = settings.restart_on_error;
        
        tokio::spawn(async move {
            tracing::info!("starting indexer");
            let mut backoff = Duration::from_millis(restart_interval);
            loop {
                if let Err(e) = indexer.run().await {
                    tracing::error!("indexer error: {}", e);
                    if restart_on_error {
                        tokio::time::sleep(backoff).await;
                        backoff = backoff * 2;
                    }
                }
            }
        });
    }

    let router = Router {
        cctx,
        health,
        stats,
        token_info,
        channel,
    };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
        graceful_shutdown: GracefulShutdownHandler::default(),
    };

    launcher::launch(launch_settings, http_router, grpc_router).await
}
