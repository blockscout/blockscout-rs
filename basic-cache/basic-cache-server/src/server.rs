use crate::{
    proto::{cache_actix::route_cache, health_actix::route_health, health_server::HealthServer},
    services::{CacheService, HealthService},
    settings::Settings,
};
use basic_cache_logic::db_cache::PostgresCache;
use basic_cache_proto::blockscout::basic_cache::v1::cache_server::CacheServer;
use blockscout_service_launcher::{database, launcher, launcher::LaunchSettings, tracing};

use migration::Migrator;

use std::sync::Arc;

const SERVICE_NAME: &str = "basic_cache";

#[derive(Clone)]
struct Router {
    health: Arc<HealthService>,
    cache: Arc<CacheService<basic_cache_logic::db_cache::PostgresCache>>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(CacheServer::from_arc(self.cache.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| {
            route_health(config, self.health.clone());
            route_cache(config, self.cache.clone());
        });
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());

    let db_connection = database::initialize_postgres::<Migrator>(
        &settings.database.connect.url(),
        settings.database.create_database,
        settings.database.run_migrations,
    )
    .await?;

    let cache = Arc::new(CacheService::new(PostgresCache::new(db_connection)));

    let router = Router { health, cache };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
    };

    launcher::launch(&launch_settings, http_router, grpc_router).await
}
