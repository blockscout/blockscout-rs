use crate::{
    metrics::Metrics,
    router::{configure_router, HttpRouter},
    settings::{JaegerSettings, MetricsSettings, ServerSettings, TracingSettings},
    tracing::init_logs,
};
use actix_web::{App, HttpServer};
use actix_web_prom::PrometheusMetrics;
use std::net::SocketAddr;

pub struct LaunchSettings {
    pub service_name: String,
    pub server: ServerSettings,
    pub metrics: MetricsSettings,
    pub tracing: TracingSettings,
    pub jaeger: JaegerSettings,
}

pub async fn launch<R>(
    settings: &LaunchSettings,
    http: R,
    grpc: tonic::transport::server::Router,
) -> Result<(), anyhow::Error>
where
    R: HttpRouter + Send + Sync + Clone + 'static,
{
    init_logs(&settings.service_name, &settings.tracing, &settings.jaeger);
    let metrics = Metrics::new(&settings.service_name, &settings.metrics.route);

    let mut futures = vec![];
    if settings.server.http.enabled {
        let http_server = {
            let http_server_future = http_serve(
                http,
                metrics.http_middleware().clone(),
                settings.server.http.addr,
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

    if settings.metrics.enabled {
        let addr = settings.metrics.addr;
        futures.push(tokio::spawn(async move {
            metrics.run_server(addr).await?;
            Ok(())
        }))
    }

    let (res, _, others) = futures::future::select_all(futures).await;
    for future in others.into_iter() {
        future.abort()
    }
    res?
}

fn http_serve<R>(http: R, metrics: PrometheusMetrics, addr: SocketAddr) -> actix_web::dev::Server
where
    R: HttpRouter + Send + Sync + Clone + 'static,
{
    tracing::info!("starting http server on addr {}", addr);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(metrics.clone())
            .configure(configure_router(&http))
    })
    .bind(addr)
    .unwrap_or_else(|_| panic!("failed to bind server"));

    server.run()
}

fn grpc_serve(
    grpc: tonic::transport::server::Router,
    addr: SocketAddr,
) -> impl futures::Future<Output = Result<(), tonic::transport::Error>> {
    tracing::info!("starting grpc server on addr {}", addr);
    grpc.serve(addr)
}
