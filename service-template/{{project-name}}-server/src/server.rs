use crate::{
    proto::{
        health_actix::route_health, health_server::HealthServer,
    },
    services::{
        HealthService
    },
    settings::Settings,
};
use blockscout_service_launcher::{
    {% if database -%}
    database,
    {% endif -%}
    launcher, launcher::LaunchSettings, tracing};
{% if migrations %}
use migration::Migrator;
{% endif %}
use std::sync::Arc;

const SERVICE_NAME: &str = "{{crate_name}}";

#[derive(Clone)]
struct Router {
    // TODO: add services here
    health: Arc<HealthService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());

    {% if database and migrations %}
    let _db_connection = database::initialize_postgres::<Migrator>(
        &settings.database.url,
        settings.database.create_database,
        settings.database.run_migrations,
    )
    .await?;
    {% endif %}

    // TODO: init services here

    let router = Router {
        health,
    };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
    };

    launcher::launch(&launch_settings, http_router, grpc_router).await
}
