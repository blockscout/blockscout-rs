pub use crate::settings::{BlockscoutSettings, Settings};
use crate::{
    instances::get_instances,
    proxy::{self, BlockscoutProxy},
};
use actix_cors::Cors;
use actix_web::{
    dev::Server,
    web,
    web::{Bytes, Data, Json},
    App, HttpRequest, HttpServer,
};
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub async fn handle_request(
    request: HttpRequest,
    proxy: Data<BlockscoutProxy>,
    body: Bytes,
) -> Json<proxy::Response> {
    let uri = request.uri();
    tracing::info!(uri = ?uri, "Got request");
    let responses = proxy
        .make_requests(uri.path_and_query(), body, request.head())
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
        let cors = Cors::default().allow_any_origin();
        App::new()
            .wrap(TracingLogger::default())
            .wrap(cors)
            .app_data(Data::new(proxy.clone()))
            .service(web::scope("/api/v1").route("/instances", web::get().to(get_instances)))
            .default_service(web::route().to(handle_request))
    })
    .listen(listener)?
    .run();
    Ok(server)
}
