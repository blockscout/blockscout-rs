use super::{
    metrics::Metrics,
    router::{configure_router, HttpRouter},
    settings::{MetricsSettings, ServerSettings},
    HttpServerSettings,
};
use actix_web::{middleware::Condition, App, HttpServer};
use actix_web_prom::PrometheusMetrics;
use std::net::SocketAddr;

pub struct LaunchSettings {
    pub service_name: String,
    pub server: ServerSettings,
    pub metrics: MetricsSettings,
}

pub async fn launch<R>(
    settings: &LaunchSettings,
    http: R,
    grpc: tonic::transport::server::Router,
) -> Result<(), anyhow::Error>
where
    R: HttpRouter + Send + Sync + Clone + 'static,
{
    let metrics = settings
        .metrics
        .enabled
        .then(|| Metrics::new(&settings.service_name, &settings.metrics.route));

    let mut futures = vec![];

    if settings.server.http.enabled {
        let http_server = {
            let http_server_future = http_serve(
                http,
                metrics
                    .as_ref()
                    .map(|metrics| metrics.http_middleware().clone()),
                &settings.server.http,
            );
            tokio::spawn(async move { http_server_future.await.map_err(anyhow::Error::msg) })
        };
        futures.push(http_server)
    }

    if settings.server.grpc.enabled {
        let grpc_server = {
            let grpc_server_future = grpc_serve(grpc, settings.server.grpc.addr);
            tokio::spawn(async move { grpc_server_future.await.map_err(anyhow::Error::msg) })
        };
        futures.push(grpc_server)
    }

    if let Some(metrics) = metrics {
        let addr = settings.metrics.addr;
        futures.push(tokio::spawn(async move {
            metrics.run_server(addr).await?;
            Ok(())
        }));
    }

    let (res, _, others) = futures::future::select_all(futures).await;
    for future in others.into_iter() {
        future.abort()
    }
    res?
}

fn http_serve<R>(
    http: R,
    metrics: Option<PrometheusMetrics>,
    settings: &HttpServerSettings,
) -> actix_web::dev::Server
where
    R: HttpRouter + Send + Sync + Clone + 'static,
{
    tracing::info!("starting http server on addr {}", settings.addr);

    let json_cfg = actix_web::web::JsonConfig::default().limit(settings.max_body_size);
    let cors_settings = settings.cors.clone();
    let cors_enabled = cors_settings.enabled;
    if let Some(metrics) = metrics {
        HttpServer::new(move || {
            let cors = cors_settings.clone().build();
            App::new()
                .wrap(metrics.clone())
                .wrap(Condition::new(cors_enabled, cors))
                .app_data(json_cfg.clone())
                .configure(configure_router(&http))
        })
        .bind(settings.addr)
        .expect("failed to bind server")
        .run()
    } else {
        HttpServer::new(move || {
            let cors = cors_settings.clone().build();
            App::new()
                .wrap(Condition::new(cors_enabled, cors))
                .app_data(json_cfg.clone())
                .configure(configure_router(&http))
        })
        .bind(settings.addr)
        .expect("failed to bind server")
        .run()
    }
}

fn grpc_serve(
    grpc: tonic::transport::server::Router,
    addr: SocketAddr,
) -> impl futures::Future<Output = Result<(), tonic::transport::Error>> {
    tracing::info!("starting grpc server on addr {}", addr);
    grpc.serve(addr)
}
