use crate::{service::Service, settings::Settings};
use actix_web::{web::ServiceConfig, App, HttpServer};
use stats_proto::blockscout::stats::v1::{
    stats_service_actix::route_stats_service,
    stats_service_server::{StatsService, StatsServiceServer},
};
use std::{net::SocketAddr, sync::Arc};

pub fn http_configure(config: &mut ServiceConfig, s: Arc<impl StatsService>) {
    route_stats_service(config, s);
}

pub fn http_server(s: Arc<impl StatsService>, addr: SocketAddr) -> actix_web::dev::Server {
    tracing::info!("starting http server on addr {}", addr);
    let server =
        HttpServer::new(move || App::new().configure(|config| http_configure(config, s.clone())))
            .bind(addr)
            .unwrap_or_else(|_| panic!("failed to bind server"));

    server.run()
}

pub fn grpc_server(
    s: Arc<impl StatsService>,
    addr: SocketAddr,
) -> impl futures::Future<Output = Result<(), tonic::transport::Error>> {
    tracing::info!("starting grpc server on addr {}", addr);
    let server = tonic::transport::Server::builder().add_service(StatsServiceServer::from_arc(s));
    server.serve(addr)
}

pub async fn stats(settings: Settings) -> Result<(), anyhow::Error> {
    let mut futures = vec![];

    let service = Arc::new(Service::new());

    if settings.server.http.enabled {
        let http_server = {
            let http_server_future = http_server(service.clone(), settings.server.http.addr);
            tokio::spawn(async move { http_server_future.await.map_err(anyhow::Error::msg) })
        };
        futures.push(http_server)
    }

    if settings.server.grpc.enabled {
        let grpc_server = {
            let grpc_server_future = grpc_server(service.clone(), settings.server.grpc.addr);
            tokio::spawn(async move { grpc_server_future.await.map_err(anyhow::Error::msg) })
        };
        futures.push(grpc_server)
    }

    let (res, _, others) = futures::future::select_all(futures).await;
    for future in others.into_iter() {
        future.abort()
    }
    res?
}
