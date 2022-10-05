use std::net::TcpListener;

use actix_web::{
    dev::Server,
    web,
    web::{Bytes, Data, Json},
    App, HttpRequest, HttpServer,
};
use tracing_actix_web::TracingLogger;

use crate::proxy::{self, BlockscoutProxy};
pub use crate::settings::{BlockscoutSettings, Settings};

#[tracing::instrument(skip(proxy))]
pub async fn handle_request(
    request: HttpRequest,
    proxy: Data<BlockscoutProxy>,
    body: Bytes,
) -> Json<proxy::Response> {
    let responses = proxy
        .make_requests(request.uri().path_and_query(), body, request.head())
        .await;
    Json(responses)
}

pub fn run(settings: Settings) -> Result<Server, std::io::Error> {
    let listener = TcpListener::bind(settings.server.addr)?;
    let proxy = BlockscoutProxy::new(
        settings.blockscout.instances,
        settings.blockscout.concurrent_requests,
        settings.blockscout.request_timeout,
    );

    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .app_data(Data::new(proxy.clone()))
            .default_service(web::route().to(handle_request))
    })
    .listen(listener)?
    .run();
    Ok(server)
}
