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
    websocket::{SubscriptionType, WebSocketClient, WebSocketEventBroadcaster, WebSocketManager},
};
use actix_web::{web, HttpRequest, Responder};
use blockscout_service_launcher::{
    launcher::{self, GracefulShutdownHandler, LaunchSettings},
    tracing as launcher_tracing,
};

use actix_web_actors::ws;
use sea_orm::DatabaseConnection;
use zetachain_cctx_logic::{
    client::Client,
    database::ZetachainCctxDatabase,
    events::{EventBroadcaster, NoOpBroadcaster},
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
    websocket_manager: Option<actix::Addr<WebSocketManager>>,
}

use actix::Actor;
use uuid::Uuid;
async fn ws_cctx_handler(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<String>,
    manager: web::Data<actix::Addr<WebSocketManager>>,
) -> impl Responder {
    let cctx_index = path.into_inner();
    let client_id = Uuid::new_v4();

    let websocket_client = WebSocketClient {
        client_id,
        manager: manager.get_ref().clone(),
        subscription_type: Some(SubscriptionType::CctxUpdates(cctx_index)),
    };

    ws::start(websocket_client, &req, stream)
}

async fn ws_cctxs_handler(
    req: HttpRequest,
    stream: web::Payload,
    manager: web::Data<actix::Addr<WebSocketManager>>,
) -> impl Responder {
    let client_id = Uuid::new_v4();

    let websocket_client = WebSocketClient {
        client_id,
        manager: manager.get_ref().clone(),
        subscription_type: Some(SubscriptionType::NewCctxs),
    };

    ws::start(websocket_client, &req, stream)
}

pub fn route_ws(
    config: &mut ::actix_web::web::ServiceConfig,
    websocket_manager: actix::Addr<WebSocketManager>,
) {
    config.app_data(web::Data::new(websocket_manager));
    config.route("/ws/cctxs", web::get().to(ws_cctxs_handler));
    config.route("/ws/{cctx_index}", web::get().to(ws_cctx_handler));
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

        // Only register WebSocket routes if WebSocket is enabled
        if let Some(websocket_manager) = &self.websocket_manager {
            service_config.configure(|config| route_ws(config, websocket_manager.clone()));
        }
    }
}

pub async fn run(
    settings: Settings,
    db: Arc<DatabaseConnection>,
    client: Arc<Client>,
) -> Result<(), anyhow::Error> {
    launcher_tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let database = Arc::new(ZetachainCctxDatabase::new(db.clone()));
    let health = Arc::new(HealthService::default());
    let cctx = Arc::new(CctxService::new(database.clone()));
    let stats = Arc::new(StatsService::new(database.clone()));
    let token_info = Arc::new(TokenInfoService::new(database.clone()));

    // Create WebSocket manager only if enabled
    let (websocket_manager, websocket_broadcaster) = if settings.websocket.enabled {
        let manager = WebSocketManager::default().start();
        let broadcaster =
            Arc::new(WebSocketEventBroadcaster::new(manager.clone())) as Arc<dyn EventBroadcaster>;
        (Some(manager), broadcaster)
    } else {
        let broadcaster = Arc::new(NoOpBroadcaster) as Arc<dyn EventBroadcaster>;
        (None, broadcaster)
    };

    if settings.indexer.enabled {
        let indexer = Indexer::new(
            settings.indexer.clone(),
            client,
            database,
            websocket_broadcaster.clone(),
        );
        let restart_interval = settings.restart_interval;
        let restart_on_error = settings.restart_on_error;
        
        tokio::spawn(async move {
            tracing::info!("starting indexer");
            //TODO: handle error, log it and restart the indexer
            loop {
                if let Err(e) = indexer.run().await {
                    tracing::error!("indexer error: {}", e);
                    if restart_on_error {
                        tokio::time::sleep(Duration::from_millis(restart_interval)).await;
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
        websocket_manager,
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
