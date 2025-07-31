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
{% if migrations -%}
use migration::Migrator;
{% endif -%}
use std::sync::Arc;
{% if proto_ex -%}
use crate::services::{{ProtoExName}}Impl;
use crate::proto::{{proto_ex_name}}_server::{{ProtoExName}}Server;
use crate::proto::{{proto_ex_name}}_actix::route_{{proto_ex_name}};
{% endif -%}

const SERVICE_NAME: &str = "{{crate_name}}";

#[derive(Clone)]
struct Router {
    // TODO: add services here
    health: Arc<HealthService>,
    {% if proto_ex -%}
    {{proto_ex_name}}: Arc<{{ProtoExName}}Impl>,
    {% endif -%}
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            {% if proto_ex -%}
            .add_service({{ProtoExName}}Server::from_arc(self.{{proto_ex_name}}.clone()))
            {% endif -%}
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        {% if proto_ex -%}
        service_config.configure(|config| route_{{proto_ex_name}}(config, self.{{proto_ex_name}}.clone()));
        {% endif -%}
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());

    {% if database and migrations -%}
    let _db_connection = database::initialize_postgres::<Migrator>(&settings.database).await?;
    {% endif -%}

    {% if proto_ex -%}
    let {{proto_ex_name}} = Arc::new({{ProtoExName}}Impl::default());
    {% endif -%}

    let router = Router {
        health,
        {% if proto_ex -%}
        {{proto_ex_name}},
        {% endif -%}
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
