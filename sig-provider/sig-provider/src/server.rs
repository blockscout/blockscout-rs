use crate::{fourbyte, sigeth, Settings, SignatureAggregator};
use actix_web::{web::ServiceConfig, App, HttpServer};
use sig_provider_proto::blockscout::sig_provider::v1::{
    signature_service_actix::route_signature_service,
    signature_service_server::{SignatureService, SignatureServiceServer},
};
use std::{net::SocketAddr, sync::Arc};

pub fn http_configure<S: SignatureService>(config: &mut ServiceConfig, signature: Arc<S>) {
    route_signature_service(config, signature);
}

pub fn http_server<S: SignatureService>(
    signature: Arc<S>,
    addr: SocketAddr,
) -> actix_web::dev::Server {
    tracing::info!("starting http server on addr {}", addr);
    let server = HttpServer::new(move || {
        App::new().configure(|config| http_configure(config, signature.clone()))
    })
    .bind(addr)
    .unwrap_or_else(|_| panic!("failed to bind server"));

    server.run()
}

pub fn grpc_server<S: SignatureService>(
    signature: Arc<S>,
    addr: SocketAddr,
) -> impl futures::Future<Output = Result<(), tonic::transport::Error>> {
    tracing::info!("starting grpc server on addr {}", addr);
    let server = tonic::transport::Server::builder()
        .add_service(SignatureServiceServer::from_arc(signature));

    server.serve(addr)
}

pub async fn sig_provider(settings: Settings) -> Result<(), anyhow::Error> {
    let signature = Arc::new(SignatureAggregator::new(vec![
        Arc::new(fourbyte::Source::new(settings.sources.fourbyte)),
        Arc::new(sigeth::Source::new(settings.sources.sigeth)),
    ]));

    let mut futures = vec![];

    if settings.server.http.enabled {
        let http_server = {
            let http_server_future = http_server(signature.clone(), settings.server.http.addr);
            tokio::spawn(async move { http_server_future.await.map_err(anyhow::Error::msg) })
        };
        futures.push(http_server)
    }

    if settings.server.grpc.enabled {
        let grpc_server = {
            let grpc_server_future = grpc_server(signature.clone(), settings.server.grpc.addr);
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
